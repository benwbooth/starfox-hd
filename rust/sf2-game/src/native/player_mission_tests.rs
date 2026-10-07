use super::*;
use crate::path_program::SelectedAuxiliaryState;
use crate::path_scene_state::{EncounterCoordination, EncounterObjectiveCounts};
use crate::player_action::PlayerActionState;
use crate::scene_path_world::PlayerPathRecords;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    owner: ObjectId,
}
impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let mut world = ScenePathWorld::new(RandomState::default());
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    action: Some(PlayerActionState {
                        action: None,
                        elapsed: 913,
                        auxiliary_counter: 314,
                        total_updates: 991,
                    }),
                    mission: Some(PlayerMissionControl::default()),
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0,
                        action_flags: 0xFF,
                        stored_world_position: Default::default(),
                        stored_rotation: Default::default(),
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        world.scene.player_configuration = Some(9);
        world.scene.encounter_location = Some(11);
        world.coordination = Some(EncounterCoordination::default());
        world.objective_counts = Some(EncounterObjectiveCounts::default());
        world.reticle_inhibited = Some(false);
        Self {
            objects,
            world,
            owner,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn visit(&mut self) -> Result<MissionAdmission, MissionError> {
        advance_admission(&self.objects, &mut self.world, self.owner)
    }
}

#[test]
fn mission_music_uses_the_full_location_word_and_independent_one_shot_flags() {
    let mut f = Fixture::new();
    for location in 0..=u16::MAX {
        for progress in [254, 255] {
            let flags = location.rotate_left(5) as u8;
            f.records().mission.as_mut().unwrap().flags = flags;
            f.world.scene.encounter_location = Some(location);
            f.world.coordination.as_mut().unwrap().progress = progress;
            f.world.audio.take_music_control();
            assert_eq!(f.visit(), Ok(MissionAdmission::ContinueExitControl));
            let (set, request) = if progress == 255 {
                (0x40, Some(MusicControlRequest::EncounterProgressComplete))
            } else if location != 11 {
                (0x80, Some(MusicControlRequest::EncounterProgressTransition))
            } else {
                (0, None)
            };
            assert_eq!(f.records().mission.unwrap().flags, flags | set);
            assert_eq!(
                f.world.audio.take_music_control(),
                if flags & set == 0 { request } else { None }
            );
            f.visit().unwrap();
            assert_eq!(f.world.audio.take_music_control(), None);
        }
    }
}

#[test]
fn mission_retreat_is_deferred_and_requires_objective_low_byte_inhibition_and_no_action() {
    let mut f = Fixture::new();
    f.world.scene.player_configuration = Some(0);
    f.world.scene.encounter_location = None;
    f.world.coordination = None;
    f.records().mission = None;
    for count in 0..=u16::MAX {
        for variant in 0..4 {
            let action = PlayerActionState {
                action: (variant & 2 != 0).then_some(PlayerAction::TriggeredProjectile),
                elapsed: 91,
                auxiliary_counter: 71,
                total_updates: 61,
            };
            f.records().action = Some(action);
            f.records().auxiliary.as_mut().unwrap().action_flags = count as u8;
            f.world.objective_counts.as_mut().unwrap().remaining_word = count;
            f.world.reticle_inhibited = Some(variant & 1 != 0);
            let admitted = count as u8 != 0 && variant == 1;
            assert_eq!(
                f.visit(),
                Ok(if admitted {
                    MissionAdmission::ForcedRetreatInstalled
                } else {
                    MissionAdmission::ContinueExitControl
                })
            );
            assert_eq!(
                f.records().action.unwrap(),
                if admitted {
                    PlayerActionState {
                        action: Some(PlayerAction::ForcedRetreat),
                        elapsed: 0,
                        auxiliary_counter: 0,
                        total_updates: 61,
                    }
                } else {
                    action
                }
            );
            assert_eq!(
                f.records().auxiliary.unwrap().action_flags,
                count as u8 & if admitted { 0xFE } else { 0xFF }
            );
            assert_eq!(f.world.audio.pending_music_control(), None);
        }
    }
}

#[test]
fn mission_read_gates_do_not_demand_unreached_records_or_erase_prior_music_requests() {
    let mut f = Fixture::new();
    f.world
        .audio
        .request_music_control(MusicControlRequest::ForcedRetreat);
    for configuration in 0..=255u8 {
        for progress in 0..=255u8 {
            f.world.scene.player_configuration = Some(configuration);
            f.world.coordination.as_mut().unwrap().progress = progress;
            f.records().mission.as_mut().unwrap().flags = 0xFF;
            f.world.reticle_inhibited = None;
            f.records().action = None;
            f.records().auxiliary = None;
            assert_eq!(f.visit(), Ok(MissionAdmission::ContinueExitControl));
            assert_eq!(
                f.world.audio.pending_music_control(),
                Some(MusicControlRequest::ForcedRetreat)
            );
        }
    }
    f.world.scene.player_configuration = Some(0);
    f.world.scene.encounter_location = None;
    f.world.coordination = None;
    f.records().mission = None;
    f.objects.remove(f.owner).unwrap();
    assert_eq!(f.visit(), Ok(MissionAdmission::ContinueExitControl));
}

#[test]
fn mission_missing_inputs_preserve_completed_music_and_installation_publications() {
    let mut f = Fixture::new();
    f.world.scene.player_configuration = None;
    assert_eq!(f.visit(), Err(MissionError::MissingConfiguration));
    f.world.scene.player_configuration = Some(9);
    f.world.scene.encounter_location = None;
    assert_eq!(f.visit(), Err(MissionError::MissingLocation));
    f.world.scene.encounter_location = Some(11);
    f.world.coordination = None;
    assert_eq!(f.visit(), Err(MissionError::MissingCoordination));
    f.world.coordination = Some(EncounterCoordination {
        progress: 255,
        ..Default::default()
    });
    f.records().mission = None;
    assert_eq!(f.visit(), Err(MissionError::MissingMissionControl(f.owner)));
    f.records().mission = Some(PlayerMissionControl::default());
    f.world.objective_counts = None;
    assert_eq!(f.visit(), Err(MissionError::MissingObjectiveState));
    assert_eq!(f.records().mission.unwrap().flags, 0x40);
    assert_eq!(
        f.world.audio.take_music_control(),
        Some(MusicControlRequest::EncounterProgressComplete)
    );
    f.world.objective_counts = Some(EncounterObjectiveCounts {
        remaining_word: 1,
        ..Default::default()
    });
    f.world.reticle_inhibited = None;
    assert_eq!(f.visit(), Err(MissionError::MissingSceneInhibition));
    f.world.reticle_inhibited = Some(true);
    f.records().action = None;
    assert_eq!(f.visit(), Err(MissionError::MissingAction(f.owner)));
    f.records().action = Some(PlayerActionState::default());
    f.records().auxiliary = None;
    assert_eq!(
        f.visit(),
        Err(MissionError::World(WorldInputError::MissingAuxiliary(
            f.owner
        )))
    );
    assert_eq!(
        f.records().action.unwrap().action,
        Some(PlayerAction::ForcedRetreat)
    );
    assert_eq!(f.world.audio.pending_music_control(), None);
}
