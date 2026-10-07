//! Exit helper failure ordering and shared-publication preservation.
use super::tests::setup;
use super::*;
use crate::path_exit::ExitCraftPose;
use crate::path_fields::ByteField;
use crate::path_scene_state::{CameraTrackingTarget, EncounterHandoff, HandoffCommand};
use crate::{Angle, Vector3};

fn cursor(index: u16) -> PathCursor {
    PathCursor {
        path: crate::PathId::from_catalog_index(0),
        command_index: index,
    }
}

const POSES: [ExitCraftPose; 8] = [ExitCraftPose {
    offset: Vector3 {
        x: 31,
        y: -72,
        z: 97,
    },
    rotation: super::super::Rotation {
        pitch: Angle::ZERO,
        yaw: Angle::ZERO,
        roll: Angle::ZERO,
    },
    relative_pitch: Angle::from_units(17),
    turn_rate: 249,
    relative_roll: Angle::from_units(32),
    target_speed: 25,
}; 8];

#[test]
fn reviewed_optional_retirement_allows_absent_parent_but_still_retires_a_real_child() {
    use crate::path_relationships::{
        apply, attach_fresh_child, RelationshipCommand, RelationshipError,
    };
    let (_, mut objects, owner, _) = setup();
    let before = objects.clone();
    for command in [
        RelationshipCommand::RetireChild { number: 60 },
        RelationshipCommand::RetireOptionalChild {
            number: 60,
            allow_absent_parent: false,
        },
    ] {
        assert_eq!(
            apply(&mut objects, owner, command),
            Err(RelationshipError::MissingParent(owner))
        );
        assert_eq!(objects, before);
    }
    let command = RelationshipCommand::RetireOptionalChild {
        number: 60,
        allow_absent_parent: true,
    };
    apply(&mut objects, owner, command).unwrap();
    assert_eq!(objects, before);
    let child = objects
        .allocate(super::super::Object::new(
            super::super::ObjectKind::Effect,
            super::super::ShapeId::EMPTY,
            super::super::Behavior::Unassigned,
        ))
        .unwrap();
    attach_fresh_child(&mut objects, owner, child, 60).unwrap();
    let mut expected = objects.clone();
    expected
        .get_mut(child)
        .unwrap()
        .base
        .flags
        .remove_after_tick = true;
    apply(&mut objects, owner, command).unwrap();
    assert_eq!(objects, expected);
    objects.get_mut(child).unwrap().base.child_number = 59;
    objects.get_mut(child).unwrap().base.attachment_next = Some(child);
    assert_eq!(
        apply(&mut objects, owner, command),
        Err(RelationshipError::ChildCycle(child))
    );
}

#[test]
fn exit_initializer_publishes_speed_before_missing_handoff_without_advancing() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    let catalog = PathCatalog::new(vec![vec![Statement::InitializeExitCraft {
        poses: &POSES,
        next: cursor(1),
    }]])
    .unwrap();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
    let mut expected = objects.clone();
    expected.get_mut(owner).unwrap().base.speed = 50;
    let mut world = PathWorld::unbound(&mut random, 0);
    assert_eq!(
        runtime.step_program(&catalog, &mut objects, owner, &mut world),
        Err(ProgramError::MissingEncounterHandoff)
    );
    assert_eq!(objects, expected);
}

#[test]
fn exit_view_keeps_initial_placement_on_late_missing_inputs_and_uses_last_spawn() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    let target = objects
        .allocate(super::super::Object::new(
            super::super::ObjectKind::Effect,
            super::super::ShapeId::EMPTY,
            super::super::Behavior::Unassigned,
        ))
        .unwrap();
    let catalog = PathCatalog::new(vec![vec![Statement::PositionExitView {
        depths: &[-15, -10, 0, 10, 20, 10, 0, -10],
        next: cursor(1),
    }]])
    .unwrap();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
    objects.get_mut(owner).unwrap().base.position = Vector3 {
        x: 5,
        y: -17,
        z: 32000,
    };
    objects.get_mut(owner).unwrap().base.yaw = Angle::ZERO;
    objects.get_mut(target).unwrap().base.pitch = Angle::from_units(31);
    let mut world = PathWorld::unbound(&mut random, 0);
    let before = objects.clone();
    assert_eq!(
        runtime.step_program(&catalog, &mut objects, owner, &mut world),
        Err(ProgramError::ActorContext(
            ActorContextError::MissingLastSpawn
        ))
    );
    assert_eq!(objects, before);
    runtime.spawns.last_spawn = Some(target);
    // A different camera tracking actor cannot redirect last-spawn placement.
    let mut tracking = CameraTrackingTarget { actor: Some(owner) };
    world.camera_tracking = Some(&mut tracking);
    assert_eq!(
        runtime.step_program(&catalog, &mut objects, owner, &mut world),
        Err(ProgramError::MissingEncounterHandoff)
    );
    let view = objects.get(target).unwrap();
    // Source Q15 rotation truncates an authored depth of 80 to 79 at yaw zero.
    assert_eq!(
        view.base.position,
        Vector3 {
            x: 5,
            y: 623,
            z: -28480
        }
    );
    assert_eq!(view.base.pitch, Angle::from_units(31));
    assert_eq!(objects.get(owner), before.get(owner));
    let mut handoff = EncounterHandoff {
        heading_word: 0xFF77,
        ..Default::default()
    };
    world.handoff = Some(&mut handoff);
    let after = objects.clone();
    assert_eq!(
        runtime.step_program(&catalog, &mut objects, owner, &mut world),
        Err(ProgramError::MissingSceneByte(SceneByte::EntryHeading))
    );
    assert_eq!(objects, after);
    world.scene.entry_heading = Some(64);
    let _ = runtime
        .step_program(&catalog, &mut objects, owner, &mut world)
        .unwrap();
    assert_eq!(objects.get(target).unwrap().base.yaw, Angle::from_units(64));
    assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(1)));
}

#[test]
fn tracking_import_is_a_reference_copy_not_attachment_creation_or_validation() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    let catalog = PathCatalog::new(vec![vec![Statement::AttachCameraTrackingTarget {
        next: cursor(1),
    }]])
    .unwrap();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
    let mut world = PathWorld::unbound(&mut random, 0);
    let before = objects.clone();
    assert_eq!(
        runtime.step_program(&catalog, &mut objects, owner, &mut world),
        Err(ProgramError::MissingCameraTrackingTarget)
    );
    assert_eq!(objects, before);
    let mut tracking = CameraTrackingTarget::default();
    for reference in [Some(owner), None] {
        objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
        tracking.actor = reference;
        let mut world = PathWorld::unbound(&mut random, 0);
        world.camera_tracking = Some(&mut tracking);
        let _ = runtime
            .step_program(&catalog, &mut objects, owner, &mut world)
            .unwrap();
        let mut expected = before.clone();
        let actor = expected.get_mut(owner).unwrap();
        actor.base.attachment = reference;
        actor.base.path = Some(cursor(1));
        assert_eq!(objects, expected);
        world.camera_tracking = None;
    }
}

#[test]
fn exit_handoff_requests_or_only_their_owned_bits_and_import_the_full_direction_byte() {
    let (_, mut objects, owner, _) = setup();
    for bits in 0..=u8::MAX {
        for (command, mask) in [
            (HandoffCommand::RequestLayoutAdvance, 1),
            (HandoffCommand::PublishExitViewReady, 4),
            (HandoffCommand::RequestCorridorExit, 8),
        ] {
            let mut handoff = EncounterHandoff {
                player_flags: bits,
                x: 171,
                z: -347,
                heading_word: u16::from_le_bytes([!bits, bits]),
            };
            let mut expected = handoff;
            expected.player_flags |= mask;
            handoff.apply(objects.get_mut(owner).unwrap(), command);
            assert_eq!(handoff, expected);
            handoff.apply(
                objects.get_mut(owner).unwrap(),
                HandoffCommand::CopyExitDirection(ByteField::AttackPower),
            );
            assert_eq!(objects.get(owner).unwrap().base.attack_power, bits);
            assert_eq!(handoff, expected);
        }
    }
}
