//! Scene-path publications added for indexed scene five: background scroll
//! words, the projected-camera import and the view-rotation copy.
use super::tests::setup;
use super::*;
use crate::path_control::PlayerTarget;
use crate::path_fields::WordField;
use crate::path_invocation::InvocationWorld;
use crate::scene_path_world::ScenePathWorld;
use crate::view_transition::{FixedViewAngles, FixedViewCommand};
use crate::{Behavior, ObjectKind, ShapeId};

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: crate::PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn run(
    statements: Vec<Statement>,
    world: &mut ScenePathWorld,
) -> (
    PathRuntime,
    ObjectStore,
    ObjectId,
    Result<PathStep, ProgramError>,
) {
    let (mut runtime, mut objects, owner, _) = setup();
    let count = statements.len();
    let catalog = PathCatalog::new(vec![statements]).unwrap();
    objects.get_mut(owner).unwrap().base.path = Some(at(0));
    let mut borrowed = world
        .path_world(&objects, owner, PlayerTarget::Primary)
        .unwrap();
    let result = runtime
        .enter_program(&catalog, &mut objects, owner, &mut borrowed, count + 1)
        .map(|exit| exit.step);
    (runtime, objects, owner, result.map_err(|e| e))
}

type PathStep = crate::path_commands::ControlStep;

#[test]
fn background_words_are_published_to_the_runtime_not_the_actor() {
    let mut world = ScenePathWorld::new(crate::RandomState::default());
    let (runtime, objects, owner, _) = run(
        vec![
            Statement::SetBackgroundHorizontal {
                value: -123,
                next: at(1),
            },
            Statement::PublishBackgroundScrollShadow {
                source: WordField::ScriptValue,
                next: at(2),
            },
        ],
        &mut world,
    );
    assert_eq!(runtime.background_horizontal, Some(-123));
    assert_eq!(runtime.background_scroll_shadow, Some(0));
    assert_eq!(objects.get(owner).unwrap().base.path, Some(at(2)));
}

#[test]
fn camera_projection_import_faults_when_unpublished_and_copies_the_word_when_published() {
    let statements = || {
        vec![Statement::ImportCameraProjection {
            destination: WordField::ScriptValue,
            next: at(1),
        }]
    };
    let mut world = ScenePathWorld::new(crate::RandomState::default());
    let (_, objects, owner, result) = run(statements(), &mut world);
    assert_eq!(result, Err(ProgramError::MissingCameraProjection));
    assert_eq!(objects.get(owner).unwrap().base.path, Some(at(0)));
    world.published_camera_projection = Some(-17_000);
    let (_, objects, owner, _) = run(statements(), &mut world);
    let actor = objects.get(owner).unwrap();
    assert_eq!(actor.extension.path_state.script_value, (-17_000i16) as u16);
    assert_eq!(actor.base.path, Some(at(1)));
}

#[test]
fn view_rotation_copy_takes_only_the_three_angle_high_bytes() {
    let (mut runtime, mut objects, owner, _) = setup();
    let mut view = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
    FixedViewAngles {
        pitch: 0x12AB,
        yaw: 0x34CD,
        roll: 0x56EF,
    }
    .write_to(&mut view);
    let view = objects.allocate(view).unwrap();
    let mut world = ScenePathWorld::new(crate::RandomState::default());
    world.fixed_players[0] = Some(view);
    let catalog = PathCatalog::new(vec![vec![Statement::FixedView {
        command: FixedViewCommand::CopyRotationFromView,
        next: at(1),
    }]])
    .unwrap();
    objects.get_mut(owner).unwrap().base.path = Some(at(0));
    let view_before = objects.get(view).unwrap().clone();
    let mut borrowed = world
        .path_world(&objects, owner, PlayerTarget::Primary)
        .unwrap();
    let _ = runtime.enter_program(&catalog, &mut objects, owner, &mut borrowed, 1);
    let actor = objects.get(owner).unwrap();
    assert_eq!(
        (
            actor.base.pitch.units(),
            actor.base.yaw.units(),
            actor.base.roll.units()
        ),
        (0x12, 0x34, 0x56)
    );
    assert_eq!(objects.get(view).unwrap(), &view_before);
}
