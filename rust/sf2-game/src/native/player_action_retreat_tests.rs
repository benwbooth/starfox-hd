use super::*;
use crate::path_scene_state::SceneTransitionControl;
use crate::path_sound::MusicControlRequest;
use crate::view_blend::ViewBlendControl;

fn fixture() -> Scene {
    let mut scene = Scene::new();
    scene.action().install(PlayerAction::ForcedRetreat);
    let record = scene.world.player_mut(&scene.objects, scene.owner).unwrap();
    record.contact = Some(Default::default());
    record.auxiliary = Some(crate::path_program::SelectedAuxiliaryState {
        mode: 0,
        action_flags: 0,
        stored_world_position: Default::default(),
        stored_rotation: Default::default(),
    });
    record.camera_auxiliary = Some(Default::default());
    scene.world.scene_transition = Some(SceneTransitionControl { phase_word: 0xA5F7 });
    scene.world.player_view_options_enabled = Some(true);
    let view = scene
        .objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    scene.world.fixed_players[0] = Some(view);
    scene
}

#[test]
fn retreat_refreshes_protection_at_every_clock_word_including_saturation() {
    let mut scene = fixture();
    let view = scene.world.fixed_players[0].unwrap();
    for time in 0..=u16::MAX {
        *scene.action() = PlayerActionState {
            action: Some(PlayerAction::ForcedRetreat),
            elapsed: time,
            auxiliary_counter: 0xFEDC,
            total_updates: u16::MAX,
        };
        let record = scene.world.player_mut(&scene.objects, scene.owner).unwrap();
        record.contact.as_mut().unwrap().hit.secondary_protection = time as u8;
        record.contact.as_mut().unwrap().hit.recovery = (time >> 8) as u8;
        record.camera_auxiliary.as_mut().unwrap().task = AuxiliaryCameraTask::Handoff;
        ViewBlendControl::default().write_to(scene.objects.get_mut(view).unwrap());
        scene.visit(false).unwrap();
        let record = scene.world.player(&scene.objects, scene.owner).unwrap();
        assert_eq!(record.contact.unwrap().hit.secondary_protection, 63);
        assert_eq!(record.contact.unwrap().hit.recovery, (time >> 8) as u8);
        assert_eq!(
            record.camera_auxiliary.unwrap().task,
            if time == 8 {
                AuxiliaryCameraTask::Initialize(OrbitStyle::Retreat)
            } else {
                AuxiliaryCameraTask::Handoff
            }
        );
        let view = ViewBlendControl::capture(scene.objects.get(view).unwrap());
        assert_eq!(view.capture_position, time == 8);
        assert_eq!(view.capture_rotation, time == 8);
        assert_eq!(scene.action().elapsed, time.saturating_add(1));
        assert_eq!(
            scene.action().total_updates,
            if time == u16::MAX { u16::MAX } else { 0 }
        );
        assert_eq!(scene.action().auxiliary_counter, 0xFEDC);
    }
}

#[test]
fn retreat_initial_publications_preserve_companion_and_unrelated_action_flags() {
    let mut scene = fixture();
    for word in 0..=u16::MAX {
        scene.action().elapsed = 0;
        scene.world.scene_transition.as_mut().unwrap().phase_word = word;
        scene.world.player_view_options_enabled = Some(true);
        scene
            .world
            .player_mut(&scene.objects, scene.owner)
            .unwrap()
            .auxiliary
            .as_mut()
            .unwrap()
            .action_flags = word as u8;
        scene.visit(false).unwrap();
        assert_eq!(
            scene.world.scene_transition.unwrap().phase_word,
            (word & 0xFF00) | 6
        );
        assert_eq!(scene.world.player_view_options_enabled, Some(false));
        assert_eq!(
            scene
                .world
                .player(&scene.objects, scene.owner)
                .unwrap()
                .auxiliary
                .unwrap()
                .action_flags,
            word as u8 & 0xFE
        );
        assert_eq!(
            scene.world.audio.take_music_control(),
            Some(MusicControlRequest::ForcedRetreat)
        );
        assert_eq!(scene.world.audio.take_music_control(), None);
    }
    // These inputs are used only on the first visit, never synthesized later.
    scene.world.scene_transition = None;
    scene
        .world
        .player_mut(&scene.objects, scene.owner)
        .unwrap()
        .auxiliary = None;
    scene
        .world
        .player_mut(&scene.objects, scene.owner)
        .unwrap()
        .camera_auxiliary = None;
    scene.world.fixed_players[0] = None;
    scene.world.player_view_options_enabled = None;
    scene.visit(false).unwrap();
    assert_eq!(scene.world.audio.pending_music_control(), None);
    assert_eq!(scene.world.player_view_options_enabled, None);
}

#[test]
fn retreat_failures_keep_the_source_order_and_scene_wrapper_prevents_replay() {
    for fault in 0..4 {
        let mut scene = fixture();
        scene
            .world
            .player_mut(&scene.objects, scene.owner)
            .unwrap()
            .auxiliary
            .as_mut()
            .unwrap()
            .action_flags = 0xFF;
        let expected = match fault {
            0 => {
                scene
                    .world
                    .player_mut(&scene.objects, scene.owner)
                    .unwrap()
                    .contact = None;
                PlayerActionError::World(WorldInputError::MissingPlayerContact(scene.owner))
            }
            1 => {
                scene
                    .world
                    .player_mut(&scene.objects, scene.owner)
                    .unwrap()
                    .auxiliary = None;
                PlayerActionError::World(WorldInputError::MissingAuxiliary(scene.owner))
            }
            2 => {
                scene.world.scene_transition = None;
                PlayerActionError::MissingSceneTransition
            }
            _ => {
                scene.action().elapsed = 8;
                scene.world.fixed_players[0] = None;
                PlayerActionError::Camera(AuxiliaryCameraError::MissingFixedView)
            }
        };
        let before = *scene.action();
        assert_eq!(scene.visit(false), Err(SceneError::PlayerAction(expected)));
        assert_eq!(*scene.action(), before);
        assert_eq!(scene.world.player_view_options_enabled, Some(fault == 3));
        assert_eq!(scene.world.audio.pending_music_control(), None);
        let record = scene.world.player(&scene.objects, scene.owner).unwrap();
        if fault != 0 {
            assert_eq!(record.contact.unwrap().hit.secondary_protection, 63);
        }
        if fault == 2 {
            assert_eq!(record.auxiliary.unwrap().action_flags, 0xFE);
        }
        if fault == 3 {
            assert_eq!(
                record.camera_auxiliary.unwrap().task,
                AuxiliaryCameraTask::Initialize(OrbitStyle::Retreat)
            );
        }
        assert_eq!(scene.visit(false), Err(SceneError::Faulted));
    }
}

#[test]
fn switching_streams_resets_local_clocks_only_and_audio_control_does_not_consume_cues() {
    let mut scene = fixture();
    *scene.action() = PlayerActionState {
        action: Some(PlayerAction::ForcedRetreat),
        elapsed: 90,
        auxiliary_counter: 71,
        total_updates: 333,
    };
    let before = *scene.action();
    scene.action().install(PlayerAction::ForcedRetreat);
    assert_eq!(*scene.action(), before);
    scene.action().install_triggered_projectile();
    assert_eq!(scene.action().elapsed, 0);
    assert_eq!(scene.action().auxiliary_counter, 0);
    assert_eq!(scene.action().total_updates, 333);
    scene.action().elapsed = 37;
    scene.action().install(PlayerAction::ForcedRetreat);
    assert_eq!(scene.action().elapsed, 0);
    scene.world.audio.queue(crate::SoundEvent::RapidLaser);
    scene.visit(false).unwrap();
    scene
        .world
        .audio
        .request_music_control(MusicControlRequest::ForcedRetreat);
    assert_eq!(
        scene.world.audio.take_events()[0],
        Some(crate::SoundEvent::RapidLaser)
    );
    assert_eq!(
        scene.world.audio.take_music_control(),
        Some(MusicControlRequest::ForcedRetreat)
    );
    assert_eq!(scene.world.audio.take_music_control(), None);
}

#[test]
fn mission_admission_wrapper_latches_failure_after_action_installation() {
    let mut scene = fixture();
    scene.action().action = None;
    scene.world.reticle_inhibited = Some(true);
    scene.world.objective_counts = Some(crate::path_scene_state::EncounterObjectiveCounts {
        remaining_word: 1,
        ..Default::default()
    });
    scene
        .world
        .player_mut(&scene.objects, scene.owner)
        .unwrap()
        .auxiliary = None;
    let catalog = PathCatalog::new(vec![]).unwrap();
    let result = SceneActors {
        objects: &mut scene.objects,
        world: &mut scene.world,
        execution: &mut scene.execution,
        catalog: &catalog,
        callbacks: &mut Callbacks,
        statement_budget: 50,
    }
    .advance_player_mission_admission(scene.owner);
    assert_eq!(
        result,
        Err(SceneError::PlayerMission(
            crate::player_mission::MissionError::World(WorldInputError::MissingAuxiliary(
                scene.owner
            ))
        ))
    );
    assert_eq!(scene.action().action, Some(PlayerAction::ForcedRetreat));
    assert_eq!(scene.visit(false), Err(SceneError::Faulted));
}
