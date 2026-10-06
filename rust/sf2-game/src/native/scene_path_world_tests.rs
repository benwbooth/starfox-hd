use super::*;
use crate::path_commands::ControlCommand;
use crate::path_invocation::{InvocationEntry, InvocationError, PathInvocation};
use crate::path_program::{PathCatalog, ProgramError, Statement};
use crate::path_shots::ShotCountCommand;
use crate::{authored_paths, Behavior, Object, ObjectKind, PathCursor, PathId, ShapeId, Vector3};

fn cursor(path: u16, command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(path),
        command_index,
    }
}

fn actor(objects: &mut ObjectStore, path: Option<PathCursor>) -> ObjectId {
    let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
    actor.base.path = path;
    actor.base.hit_points = 1;
    objects.allocate(actor).unwrap()
}

#[test]
fn selected_player_and_linked_shot_count_resolve_independent_live_owners() {
    let mut objects = ObjectStore::new();
    let primary = actor(&mut objects, None);
    let secondary = actor(&mut objects, None);
    let owner = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::new([1, 2, 3, 4]));
    world.primary_player = Some(primary);
    world.secondary_player = Some(secondary);
    world.active_charge_threshold = Some(25);
    for (player, mode, count) in [(primary, 17, 2), (secondary, 35, 6)] {
        objects.get_mut(player).unwrap().extension.path_state.motion_delta =
            Vector3 { x: 27, y: -51, z: 77 };
        world
            .bind_player(
                &objects,
                player,
                PlayerPathRecords {
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode,
                        action_flags: 0,
                        stored_world_position: Vector3::default(),
                        stored_rotation: Rotation::default(),
                    }),
                    suppress_horizontal_follow: Some(true),
                    equipment: Some(SelectedEquipment::default()),
                    ..PlayerPathRecords::default()
                },
            )
            .unwrap();
        world
            .bind_shots(&objects, player, ActiveShots::from_count(count))
            .unwrap();
    }
    for (selected, linked) in [
        (PlayerTarget::Primary, secondary),
        (PlayerTarget::Secondary, primary),
        (PlayerTarget::Primary, primary),
    ] {
        objects.get_mut(owner).unwrap().base.attachment = Some(linked);
        let selected_owner = world.selected(selected).unwrap();
        let selected_mode = world
            .player_mut(&objects, selected_owner)
            .unwrap()
            .auxiliary
            .unwrap()
            .mode;
        let previous_count = world.shots(&objects, linked).unwrap().count();
        let mut input = world.path_world(&objects, owner, selected).unwrap();
        assert_eq!(input.selected, Some(selected_owner));
        assert_eq!(
            input.selected_auxiliary.as_ref().unwrap().mode,
            selected_mode
        );
        assert_eq!(input.primary_motion.unwrap().auxiliary_mode, 17);
        assert_eq!(input.primary_motion.unwrap().displacement.y, -51);
        assert_eq!(input.active_charge_threshold, Some(25));
        input.selected_equipment.as_mut().unwrap().weapon_level = 3;
        input
            .linked_shot_count
            .as_mut()
            .unwrap()
            .state
            .apply(ShotCountCommand::Increment);
        drop(input);
        assert_eq!(
            world.shots(&objects, linked).unwrap().count(),
            previous_count + 1
        );
        assert_eq!(
            world
                .player_mut(&objects, selected_owner)
                .unwrap()
                .equipment
                .unwrap()
                .weapon_level,
            3
        );
    }
}

#[test]
fn recycled_slots_cannot_inherit_auxiliary_shots_displacement_or_carry() {
    let mut objects = ObjectStore::new();
    let player = actor(&mut objects, None);
    let owner = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(player);
    world
        .bind_player(
            &objects,
            player,
            PlayerPathRecords {
                equipment: Some(SelectedEquipment::default()),
                suppress_horizontal_follow: Some(false),
                carried: Some(CarriedPlayer::default()),
                ..PlayerPathRecords::default()
            },
        )
        .unwrap();
    world
        .bind_shots(&objects, player, ActiveShots::from_count(7))
        .unwrap();
    objects.get_mut(owner).unwrap().base.attachment = Some(player);
    let previous = objects.lifetime_id(player).unwrap();
    objects.remove(player).unwrap();
    let replacement = actor(&mut objects, None);
    assert_eq!(replacement, player);
    assert_ne!(objects.lifetime_id(replacement), Some(previous));
    objects.get_mut(owner).unwrap().base.attachment = Some(replacement);
    objects
        .get_mut(replacement)
        .unwrap()
        .base
        .flags
        .standing_on_surface = true;
    objects
        .get_mut(replacement)
        .unwrap()
        .extension
        .surface_contact
        .supporting_object = Some(owner);
    assert_eq!(
        world.player_mut(&objects, player),
        Err(WorldInputError::StalePlayerRecord(player))
    );
    assert_eq!(world.shots(&objects, player), None);
    assert_eq!(
        world.displacement(&objects, PlayerTarget::Primary),
        Err(WorldInputError::StalePlayerRecord(player))
    );
    assert_eq!(
        world.carried_player(&objects, owner, PlayerTarget::Primary),
        Err(WorldInputError::StalePlayerRecord(player))
    );
    let inputs = world
        .path_world(&objects, owner, PlayerTarget::Primary)
        .unwrap();
    assert!(inputs.selected_equipment.is_none());
    assert!(inputs.linked_shot_count.is_none());
}

#[test]
fn missing_publication_faults_at_the_statement_and_can_resume_without_replaying() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects, Some(cursor(0, 0)));
    let mut world = ScenePathWorld::new(RandomState::new([11, 17, 23, 29]));
    let catalog = PathCatalog::new(vec![vec![
        Statement::SceneEvent {
            command: super::super::path_scene_state::SceneEventCommand::Assign(
                crate::path_fields::WordOperand::Literal(255),
            ),
            next: cursor(0, 1),
        },
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    let mut invocation = PathInvocation::default();
    invocation.begin(owner, InvocationEntry::Program).unwrap();
    let before = objects.clone();
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut world, 32),
        Err(InvocationError::Program(ProgramError::MissingSceneEvents))
    );
    assert_eq!(objects, before);
    world.scene_events = Some(SceneEventFlags { bits: 0xAF00 });
    assert_eq!(
        invocation.resume(&catalog, &mut objects, &mut world, 32),
        Ok(owner)
    );
    assert_eq!(world.scene_events.unwrap().bits, 255);
    assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
    assert_eq!(world.random.bytes(), [11, 17, 23, 29]);
}

#[test]
fn crossing_inputs_use_live_actor_and_player_transforms_but_local_predicates_need_no_player() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects, None);
    let player = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.strategy_clock = 0xBCFE;
    let local = world
        .trigger_inputs(&objects, owner, PlayerTarget::Primary, TriggerKind::Always)
        .unwrap();
    assert_eq!(local.strategy_tick, 254);
    assert_eq!(local.player_projections, [None; 2]);
    assert_eq!(
        world.trigger_inputs(
            &objects,
            owner,
            PlayerTarget::Primary,
            TriggerKind::ControlledAuxFlagHigh
        ),
        Err(WorldInputError::MissingSelectedPlayer(
            PlayerTarget::Primary
        ))
    );
    world.primary_player = Some(player);
    objects.get_mut(player).unwrap().base.position.z = 100;
    let first = world
        .trigger_inputs(
            &objects,
            owner,
            PlayerTarget::Secondary,
            TriggerKind::PlayerCrossing,
        )
        .unwrap();
    // The projection leaf has separate source-arithmetic tests. This test
    // checks that each binding uses the current poses, not a cached snapshot.
    assert_eq!(
        first.player_projections,
        [
            Some(forward_plane_projection(
                Vector3::default(),
                Rotation::default(),
                Vector3 {
                    z: 100,
                    ..Vector3::default()
                },
            )),
            None
        ]
    );
    assert!(first.player_projections[0].unwrap() > 0);
    objects.get_mut(owner).unwrap().base.position.z = 200;
    let second = world
        .trigger_inputs(
            &objects,
            owner,
            PlayerTarget::Secondary,
            TriggerKind::PlayerCrossing,
        )
        .unwrap();
    assert_eq!(
        second.player_projections,
        [
            Some(forward_plane_projection(
                Vector3 {
                    z: 200,
                    ..Vector3::default()
                },
                Rotation::default(),
                Vector3 {
                    z: 100,
                    ..Vector3::default()
                },
            )),
            None
        ]
    );
    assert!(second.player_projections[0].unwrap() < 0);
    objects.get_mut(owner).unwrap().base.yaw = Angle::from_units(128);
    let turned = world
        .trigger_inputs(
            &objects,
            owner,
            PlayerTarget::Secondary,
            TriggerKind::PlayerCrossing,
        )
        .unwrap();
    assert_eq!(
        turned.player_projections,
        [
            Some(forward_plane_projection(
                Vector3 {
                    z: 200,
                    ..Vector3::default()
                },
                Rotation {
                    yaw: Angle::HALF_TURN,
                    ..Rotation::default()
                },
                Vector3 {
                    z: 100,
                    ..Vector3::default()
                },
            )),
            None
        ]
    );
    assert!(turned.player_projections[0].unwrap() > 0);
}

#[test]
fn authored_charge_path_runs_through_scene_owned_selected_state_and_retained_callbacks() {
    let mut objects = ObjectStore::new();
    let player = actor(&mut objects, None);
    let owner = actor(&mut objects, Some(authored_paths::PLAYER_CHARGE_ORB));
    let mut world = ScenePathWorld::new(RandomState::new([11, 17, 23, 29]));
    world.primary_player = Some(player);
    world.active_charge_threshold = Some(25);
    world
        .bind_player(
            &objects,
            player,
            PlayerPathRecords {
                charge: Some(crate::player_charge::PlayerCharge {
                    linked_mode: false,
                    progress: 3 << 8,
                    ..Default::default()
                }),
                ..PlayerPathRecords::default()
            },
        )
        .unwrap();
    let catalog = authored_paths::catalog();
    let mut invocation = PathInvocation::default();
    for visit in 0..16 {
        world.strategy_clock += 1;
        if visit == 10 {
            world
                .player_mut(&objects, player)
                .unwrap()
                .charge
                .as_mut()
                .unwrap()
                .progress = 25 << 8;
        }
        let held = objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .hold_latched;
        invocation
            .begin(
                owner,
                if held {
                    InvocationEntry::Movement
                } else {
                    InvocationEntry::Program
                },
            )
            .unwrap();
        assert_eq!(
            invocation.resume(&catalog, &mut objects, &mut world, 64),
            Ok(owner)
        );
        let actor = objects.get(owner).unwrap();
        if visit >= 2 {
            assert_eq!(actor.extension.parent, Some(player));
            assert_eq!(
                actor.extension.path_state.motion_phase as u8,
                if visit < 10 { 3 } else { 25 }
            );
        }
        assert_eq!(actor.extension.path_state.hold_latched, visit >= 11);
        assert_eq!(
            actor.base.shape,
            ShapeId::from_catalog_index(if visit < 2 {
                0
            } else if visit < 11 {
                15
            } else {
                17
            })
        );
        assert!(!actor.base.flags.remove_after_tick);
    }
    assert_eq!(world.random.bytes(), [11, 17, 23, 29]);
    assert!(invocation.runtime.resources.owner_count(owner) >= 2);
}

#[test]
fn part_target_uses_the_surface_alias_and_short_circuits_after_a_primary_match() {
    use crate::path_fields::ByteField;
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects, None);
    let primary = actor(&mut objects, None);
    let secondary = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(primary);
    world.secondary_player = Some(secondary);
    world
        .bind_player(
            &objects,
            primary,
            PlayerPathRecords {
                auxiliary: Some(SelectedAuxiliaryState {
                    mode: 0x27,
                    action_flags: 0,
                    stored_world_position: Vector3::default(),
                    stored_rotation: Rotation::default(),
                }),
                ..PlayerPathRecords::default()
            },
        )
        .unwrap();
    objects
        .get_mut(primary)
        .unwrap()
        .base
        .flags
        .standing_on_surface = true;
    objects
        .get_mut(primary)
        .unwrap()
        .extension
        .surface_contact
        .supporting_object = Some(owner);
    for part in 0..=u8::MAX {
        ByteField::Part.write(objects.get_mut(owner).unwrap(), part);
        objects
            .get_mut(primary)
            .unwrap()
            .extension
            .surface_contact
            .group = part;
        assert_eq!(ByteField::Part.read(objects.get(primary).unwrap()), part);
        assert_eq!(
            objects.get(owner).unwrap().extension.surface_contact.group,
            part
        );
        let input = world
            .trigger_inputs(
                &objects,
                owner,
                PlayerTarget::Secondary,
                TriggerKind::PlayerPartTarget,
            )
            .unwrap();
        assert_eq!(
            input.player_parts,
            [
                Some(PlayerPartTarget {
                    part_target_mode: true,
                    actor: Some(owner),
                    part,
                    enabled: true,
                }),
                None
            ]
        );
    }
    // Failing primary's part gate reaches the unbound secondary auxiliary.
    objects
        .get_mut(primary)
        .unwrap()
        .extension
        .surface_contact
        .group = 0;
    assert_eq!(
        world.trigger_inputs(
            &objects,
            owner,
            PlayerTarget::Primary,
            TriggerKind::PlayerPartTarget
        ),
        Err(WorldInputError::StalePlayerRecord(secondary))
    );
    let auxiliary = world.player_mut(&objects, primary).unwrap().auxiliary;
    world
        .bind_player(
            &objects,
            secondary,
            PlayerPathRecords {
                auxiliary,
                ..PlayerPathRecords::default()
            },
        )
        .unwrap();
    objects
        .get_mut(secondary)
        .unwrap()
        .extension
        .surface_contact
        .supporting_object = Some(owner);
    for mode in 0..=u8::MAX {
        world
            .player_mut(&objects, secondary)
            .unwrap()
            .auxiliary
            .as_mut()
            .unwrap()
            .mode = mode;
        let input = world
            .trigger_inputs(
                &objects,
                owner,
                PlayerTarget::Primary,
                TriggerKind::PlayerPartTarget,
            )
            .unwrap();
        let secondary = input.player_parts[1].unwrap();
        assert_eq!(secondary.part_target_mode, mode & 0xF0 == 0x20);
        assert_eq!(secondary.actor, Some(owner));
        assert!(!secondary.enabled);
    }
}

#[test]
fn carry_gates_use_live_surface_flags_before_borrowing_auxiliary_origin() {
    let mut objects = ObjectStore::new();
    let carrier = actor(&mut objects, None);
    let player = actor(&mut objects, None);
    let other = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(player);
    assert_eq!(
        world.carried_player(&objects, carrier, PlayerTarget::Primary),
        Ok(None)
    );
    objects
        .get_mut(player)
        .unwrap()
        .base
        .flags
        .standing_on_surface = true;
    objects
        .get_mut(player)
        .unwrap()
        .extension
        .surface_contact
        .supporting_object = Some(other);
    assert_eq!(
        world.carried_player(&objects, carrier, PlayerTarget::Primary),
        Ok(None)
    );
    objects
        .get_mut(player)
        .unwrap()
        .extension
        .surface_contact
        .supporting_object = Some(carrier);
    assert_eq!(
        world.carried_player(&objects, carrier, PlayerTarget::Primary),
        Err(WorldInputError::StalePlayerRecord(player))
    );
    world
        .bind_player(
            &objects,
            player,
            PlayerPathRecords {
                carried: Some(CarriedPlayer {
                    enabled: false,
                    carrier: Some(other),
                    origin: Vector3 {
                        x: 91,
                        y: -42,
                        z: 11,
                    },
                    fine_yaw: 777,
                }),
                ..PlayerPathRecords::default()
            },
        )
        .unwrap();
    let input = world
        .carried_player(&objects, carrier, PlayerTarget::Primary)
        .unwrap()
        .unwrap();
    assert!(input.enabled);
    assert_eq!(input.carrier, Some(carrier));
    assert_eq!(input.origin.x, 91);
    input.origin.x = 173;
    input.fine_yaw = 991;
    objects
        .get_mut(player)
        .unwrap()
        .base
        .flags
        .standing_on_surface = false;
    assert_eq!(
        world.carried_player(&objects, carrier, PlayerTarget::Primary),
        Ok(None)
    );
    let retained = world.player_mut(&objects, player).unwrap().carried.unwrap();
    assert_eq!(retained.origin.x, 173);
    assert_eq!(retained.fine_yaw, 991);
}

#[test]
fn primary_crossing_does_not_read_a_secondary_transform_after_its_early_exit() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects, None);
    let primary = actor(&mut objects, None);
    let secondary = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(primary);
    world.secondary_player = Some(secondary);
    objects
        .get_mut(owner)
        .unwrap()
        .extension
        .path_state
        .conditions
        .crossing
        .sample([Some(1), Some(1)]);
    objects.get_mut(primary).unwrap().base.position.z = -100;
    objects.remove(secondary).unwrap();
    let input = world
        .trigger_inputs(
            &objects,
            owner,
            PlayerTarget::Secondary,
            TriggerKind::PlayerCrossing,
        )
        .unwrap();
    assert!(input.player_projections[0].unwrap() < 0);
    assert_eq!(input.player_projections[1], None);
    objects
        .get_mut(owner)
        .unwrap()
        .extension
        .path_state
        .conditions
        .crossing = Default::default();
    // The initialization visit must sample both players' signs.
    assert_eq!(
        world.trigger_inputs(
            &objects,
            owner,
            PlayerTarget::Secondary,
            TriggerKind::PlayerCrossing
        ),
        Err(WorldInputError::MissingActor(secondary))
    );
}
