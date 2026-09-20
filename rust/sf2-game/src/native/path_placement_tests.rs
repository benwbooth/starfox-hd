use super::super::path_fields::{Axis, WordField};
use super::super::path_scene_state::{PlacementCommand, PlacementCoordinate, PlacementCoordinates};
use super::super::{Behavior, ObjectKind, PathId, ShapeId, Vector3};
use super::tests::{setup, world};
use super::*;

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

#[test]
fn unaligned_control_word_aliases_live_phase_and_script_bytes() {
    let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Effect);
    for companion in [0, 1, 127, 128, 255u8] {
        actor.extension.path_state.motion_phase = u16::from_le_bytes([companion, 201]);
        actor.extension.path_state.script_value = u16::from_le_bytes([39, !companion]);
        let before = actor.clone();
        assert_eq!(
            WordField::MotionScriptOverlap.read(&actor),
            u16::from_le_bytes([201, 39])
        );
        for bits in 0..=u16::MAX {
            WordField::MotionScriptOverlap.write(&mut actor, bits);
            assert_eq!(WordField::MotionScriptOverlap.read(&actor), bits);
            let mut expected = before.clone();
            expected.extension.path_state.motion_phase =
                u16::from(companion) | ((bits & 0xFF) << 8);
            expected.extension.path_state.script_value = (u16::from(!companion) << 8) | (bits >> 8);
            assert_eq!(actor, expected);
        }
    }
}

#[test]
fn placement_words_round_trip_all_signed_values_and_preserve_other_coordinates() {
    for coordinate in [
        PlacementCoordinate::Primary,
        PlacementCoordinate::Depth,
    ] {
        let mut placement = PlacementCoordinates::default();
        let mut actor = Object::new(ObjectKind::Scenery, ShapeId::EMPTY, Behavior::Effect);
        let before = actor.clone();
        assert_eq!(
            placement.apply(
                &mut actor,
                PlacementCommand::Import {
                    coordinate,
                    destination: WordField::Position(Axis::X),
                }
            ),
            Err(coordinate)
        );
        assert_eq!(actor, before);
        for bits in 0..=u16::MAX {
            placement.primary = Some(-79);
            placement.depth = Some(357);
            actor = before.clone();
            placement
                .apply(
                    &mut actor,
                    PlacementCommand::Export {
                        coordinate,
                        source: WordOperand::Literal(bits),
                    },
                )
                .unwrap();
            assert_eq!(actor, before);
            let expected = match coordinate {
                PlacementCoordinate::Primary => PlacementCoordinates {
                    primary: Some(bits as i16),
                    depth: Some(357),
                },
                PlacementCoordinate::Depth => PlacementCoordinates {
                    primary: Some(-79),
                    depth: Some(bits as i16),
                },
            };
            assert_eq!(placement, expected);
            placement
                .apply(
                    &mut actor,
                    PlacementCommand::Import {
                        coordinate,
                        destination: WordField::Position(Axis::Z),
                    },
                )
                .unwrap();
            let mut expected_actor = before.clone();
            expected_actor.base.position.z = bits as i16;
            assert_eq!(actor, expected_actor);
            assert_eq!(placement, expected);
        }
    }
}

#[test]
fn scenery_and_exit_placement_share_first_coordinate_across_actor_invocations() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    let second = objects
        .allocate(Object::new(
            ObjectKind::Scenery,
            ShapeId::EMPTY,
            Behavior::FollowPath,
        ))
        .unwrap();
    objects.get_mut(owner).unwrap().base.position = Vector3 {
        x: -32768,
        y: 91,
        z: 32767,
    };
    let catalog = PathCatalog::new(vec![vec![
        Statement::Placement {
            command: PlacementCommand::Export {
                coordinate: PlacementCoordinate::Primary,
                source: WordOperand::Actor(WordField::Position(Axis::X)),
            },
            next: at(1),
        },
        Statement::Placement {
            command: PlacementCommand::Export {
                coordinate: PlacementCoordinate::Depth,
                source: WordOperand::Actor(WordField::Position(Axis::Z)),
            },
            next: at(2),
        },
        Statement::ImportSceneryPlacementHeight { next: at(3) },
        Statement::SetSceneryPlacementHeight {
            height: -500,
            next: at(4),
        },
        Statement::Placement {
            command: PlacementCommand::Import {
                coordinate: PlacementCoordinate::Primary,
                destination: WordField::Position(Axis::X),
            },
            next: at(5),
        },
        Statement::Placement {
            command: PlacementCommand::Import {
                coordinate: PlacementCoordinate::Depth,
                destination: WordField::Position(Axis::Z),
            },
            next: at(6),
        },
    ]])
    .unwrap();
    let before = objects.clone();
    let random_before = random;
    runtime.branch.invert_next = true;
    let resources_before = runtime.resources.clone();
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
        Err(ProgramError::BudgetExceeded {
            cursor: at(2),
            executed: 2
        })
    );
    objects.get_mut(second).unwrap().base.path = Some(at(2));
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, second, &mut world(&mut random), 4),
        Err(ProgramError::BudgetExceeded {
            cursor: at(6),
            executed: 4
        })
    );
    let mut expected = before;
    expected.get_mut(owner).unwrap().base.path = Some(at(2));
    expected.get_mut(second).unwrap().base.path = Some(at(6));
    expected.get_mut(second).unwrap().base.position = Vector3 {
        x: -500,
        y: -32768,
        z: 32767,
    };
    assert_eq!(objects, expected);
    assert_eq!(
        runtime.placement,
        PlacementCoordinates {
            primary: Some(-500),
            depth: Some(32767)
        }
    );
    assert!(runtime.branch.invert_next);
    assert_eq!(runtime.resources, resources_before);
    assert_eq!(random, random_before);
}

#[test]
fn missing_placement_coordinate_does_not_advance_or_change_owner() {
    for coordinate in [
        PlacementCoordinate::Primary,
        PlacementCoordinate::Depth,
    ] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let before = objects.clone();
        let catalog = PathCatalog::new(vec![vec![Statement::Placement {
            command: PlacementCommand::Import {
                coordinate,
                destination: WordField::Position(Axis::X),
            },
            next: at(1),
        }]])
        .unwrap();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingPlacementCoordinate(coordinate))
        );
        assert_eq!(objects, before);
    }
}
