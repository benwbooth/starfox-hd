//! Missing-service ordering and shared state for the node-exit presentation.
use super::tests::setup;
use super::*;
use crate::path_control::PlayerTarget;
use crate::path_invocation::InvocationWorld;
use crate::path_sound::MusicControlRequest;
use crate::scene_map::{MapCatalog, MapCursor, MapInstruction, MapRestoreError, SceneMap};
use crate::scene_path_world::ScenePathWorld;

fn cursor(index: u16) -> PathCursor {
    PathCursor {
        path: crate::PathId::from_catalog_index(0),
        command_index: index,
    }
}

#[test]
fn exit_projection_reset_is_published_before_missing_view_and_does_not_advance_path() {
    let (mut runtime, mut objects, owner, random) = setup();
    let mut world = ScenePathWorld::new(random);
    let catalog = PathCatalog::new(vec![vec![Statement::DampNodeExitCamera {
        next: cursor(1),
    }]])
    .unwrap();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
    let before = objects.clone();
    let mut borrowed = world
        .path_world(&objects, owner, PlayerTarget::Primary)
        .unwrap();
    assert_eq!(
        runtime.enter_program(&catalog, &mut objects, owner, &mut borrowed, 1),
        Err(ProgramError::MissingFixedView)
    );
    assert_eq!(world.camera_projection_offset, Some(0));
    assert_eq!(objects, before);
}

#[test]
fn restore_operates_on_real_map_owner_and_never_consumes_saved_continuation() {
    let (mut runtime, mut objects, owner, random) = setup();
    let mut world = ScenePathWorld::new(random);
    let catalog = PathCatalog::new(vec![vec![
        Statement::RestoreMapContinuation { next: cursor(1) },
        Statement::Control(ControlCommand::Hold),
    ]])
    .unwrap();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
    let mut borrowed = world
        .path_world(&objects, owner, PlayerTarget::Primary)
        .unwrap();
    assert_eq!(
        runtime.enter_program(&catalog, &mut objects, owner, &mut borrowed, 1),
        Err(ProgramError::MissingSceneMap)
    );
    let program: [MapInstruction<(), ()>; 2] = [MapInstruction::Stop, MapInstruction::Stop];
    let map_catalog = MapCatalog::new(&program, &[]).unwrap();
    world.map = Some(SceneMap::new(&map_catalog, MapCursor::from_index(0)).unwrap());
    let mut borrowed = world
        .path_world(&objects, owner, PlayerTarget::Primary)
        .unwrap();
    assert_eq!(
        runtime.enter_program(&catalog, &mut objects, owner, &mut borrowed, 1),
        Err(ProgramError::MapRestore(
            MapRestoreError::MissingContinuation
        ))
    );
    world
        .map
        .as_mut()
        .unwrap()
        .save_continuation(&map_catalog, MapCursor::from_index(1))
        .unwrap();
    for _ in 0..3 {
        objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
        let mut borrowed = world
            .path_world(&objects, owner, PlayerTarget::Primary)
            .unwrap();
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut borrowed, 2)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(1)));
        let map = world.map.as_ref().unwrap();
        assert_eq!(map.cursor(), MapCursor::from_index(1));
        assert_eq!(map.saved_continuation(), Some(MapCursor::from_index(1)));
    }
}

#[test]
fn projection_base_is_a_shared_publication_but_music_requires_the_audio_owner() {
    let (mut runtime, mut objects, owner, random) = setup();
    let mut world = ScenePathWorld::new(random);
    let catalog = PathCatalog::new(vec![vec![
        Statement::SetCameraProjectionBase {
            value: 400,
            next: cursor(1),
        },
        Statement::RequestMusicControl {
            request: MusicControlRequest::EncounterExit,
            next: cursor(2),
        },
    ]])
    .unwrap();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
    let mut borrowed = world
        .path_world(&objects, owner, PlayerTarget::Primary)
        .unwrap();
    assert_eq!(
        runtime.enter_program(&catalog, &mut objects, owner, &mut borrowed, 3),
        Err(ProgramError::MissingAudio)
    );
    assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(1)));
    assert_eq!(world.camera_projection_base, Some(400));
    assert_eq!(world.audio.pending_music_control(), None);
}

#[test]
fn tracking_clears_its_shared_steering_result_before_resolving_view_and_target() {
    let (mut runtime, mut objects, owner, random) = setup();
    let mut world = ScenePathWorld::new(random);
    let catalog = PathCatalog::new(vec![vec![Statement::FixedView {
        command: crate::view_transition::FixedViewCommand::AimTracking {
            pitch_shift: 1,
            chase: false,
        },
        next: cursor(1),
    }]])
    .unwrap();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
    let before = objects.clone();
    for fixed in [None, Some(owner)] {
        runtime.steering.unchanged_axes = 0xA7;
        world.fixed_players[0] = fixed;
        let mut borrowed = world
            .path_world(&objects, owner, PlayerTarget::Primary)
            .unwrap();
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut borrowed, 1),
            Err(if fixed.is_none() {
                ProgramError::MissingFixedView
            } else {
                ProgramError::MissingCameraTrackingTarget
            })
        );
        assert_eq!(runtime.steering.unchanged_axes, 0);
        assert_eq!(objects, before);
    }
}
