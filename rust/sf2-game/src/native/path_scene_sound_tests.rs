use super::tests::{setup, world};
use super::*;
use crate::path_control::PlayerTarget;
use crate::path_sound::{AuthoredCue, CueListener, PathAudio};
use crate::{AudioState, SoundEvent};

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: crate::PathId::from_catalog_index(0),
        command_index,
    }
}

#[test]
fn scene_sound_retains_and_enqueues_every_word_without_retargeting_or_consuming_branch_state() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    let mut audio = AudioState::default();
    runtime.enter(&objects, owner).unwrap();
    runtime.branch.invert_next = true;
    let before_random = random;
    for word in 0..=u16::MAX {
        let cue = AuthoredCue::new(
            word as u8,
            (word >> 8) as u8 & 127,
            if word & 0x8000 == 0 {
                PlayerTarget::Primary
            } else {
                PlayerTarget::Secondary
            },
        );
        let catalog =
            PathCatalog::new(vec![vec![Statement::SceneSound { cue, next: at(1) }]]).unwrap();
        objects.get_mut(owner).unwrap().base.path = Some(at(0));
        let before = objects.clone();
        let mut input = world(&mut random);
        assert_eq!(
            runtime.step_program(&catalog, &mut objects, owner, &mut input),
            Err(ProgramError::MissingAudio)
        );
        assert_eq!(objects, before);
        input.audio = Some(PathAudio {
            events: &mut audio,
            listeners: [CueListener::Other; 2],
            markers: None,
        });
        assert_eq!(
            runtime.step_program(&catalog, &mut objects, owner, &mut input),
            Ok(ProgramExit { actor: owner, step: ControlStep::Continue })
        );
        assert_eq!(audio.retained_scene_cue(), Some(cue));
        assert_eq!(
            audio
                .take_events()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            vec![SoundEvent::Authored(cue)]
        );
        assert_eq!(audio.retained_scene_cue(), Some(cue));
        assert_eq!(objects.get(owner).unwrap().base.path, Some(at(1)));
        assert!(runtime.branch.invert_next);
    }
    assert_eq!(random, before_random);
}

#[test]
fn scene_sound_shares_the_wrapping_queue_but_ordinary_cues_do_not_replace_its_retained_value() {
    let mut audio = AudioState::default();
    let cue = AuthoredCue::new(250, 17, PlayerTarget::Primary);
    audio.publish_scene_cue(cue);
    for id in 1..16 {
        audio.queue(SoundEvent::Authored(AuthoredCue::new(
            id,
            0,
            PlayerTarget::Secondary,
        )));
    }
    assert!(audio.take_events().iter().all(Option::is_none));
    assert_eq!(audio.retained_scene_cue(), Some(cue));
    audio.queue(SoundEvent::RapidLaser);
    assert_eq!(audio.take_events()[0], Some(SoundEvent::RapidLaser));
    assert_eq!(audio.retained_scene_cue(), Some(cue));
}
