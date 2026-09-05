//! Original parent path in isolation: children are source-constructed but
//! their independent strategies are not run by this parent-only comparison.

use sf2_game::intro_attached_craft::{OpeningBurstAudio, OpeningBurstSound};
use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_destruction::IntroExplosionVolume;
use sf2_game::intro_motion::IntroScenePose;
use sf2_game::intro_second_flyby_craft::{
    OpeningSecondFlybyAttachmentGroup, OpeningSecondFlybyChild, OpeningSecondFlybyCraft,
    OpeningSecondFlybyEvent, OpeningSecondFlybyManeuver, OpeningSecondFlybyPhase,
    OpeningSecondFlybySound, OpeningSecondFlybySpawnPlacement,
};
use sf2_game::object::{
    active_objects, allocate, ACTIVE_LIST, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY,
    PLAYER_ONE, SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, Rotation, StereoPosition, Vector3};

const STRATEGY: u32 = 0x7F7E1E;
const VIEW: u16 = 0x033F;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("later flyby verification requires the user-owned retail SF2 ROM")
}

fn cue_at(schedule: usize, update: usize) -> (OpeningCameraCue, u8) {
    use OpeningCameraCue::*;
    match schedule {
        0 => match update + 183 {
            0..249 => (FirstCut, 2),
            249..293 => (SecondCut, 3),
            293..327 => (ThirdCut, 4),
            327..416 => (FourthCut, 5),
            _ => (FinalCut, 6),
        },
        1 => match update {
            0..100 => (SecondCut, 3),
            100..150 => (ThirdCut, 4),
            150..300 => (FourthCut, 5),
            _ => (FinalCut, 6),
        },
        2 => (Opening, 1),
        3 => (SecondCut, 3),
        4 => (ThirdCut, 4),
        _ => unreachable!(),
    }
}

fn vector(exact: &Game, actor: u16, fields: [u16; 3]) -> Vector3 {
    let [x, y, z] = fields.map(|field| exact.memory.read_word(actor + field) as i16);
    Vector3 { x, y, z }
}

fn authored_craft(rom: &[u8]) -> (Game, u16) {
    let mut exact = Game::new(rom.to_vec()).unwrap();
    let root = allocate(&mut exact.memory, 0).unwrap();
    exact.memory.write_word(root + FIELD_PATH, 0xFA11);
    exact
        .memory
        .write_word(root + FIELD_STRATEGY, STRATEGY as u16);
    exact
        .memory
        .write_byte(root + FIELD_STRATEGY + 2, (STRATEGY >> 16) as u8);
    exact.memory.write_word(root + FIELD_SHAPE, 0xBC9C);
    exact.memory.write_byte(root + 0x2D, 1);
    exact.memory.write_word(PLAYER_ONE, VIEW);
    exact.memory.write_word(SELECTED_OBJECT, VIEW);
    exact.memory.write_word(VIEW + FIELD_PATH, 0x0140);
    for update in 0..=97 {
        exact
            .memory
            .write_byte(0x1D72, if update >= 96 { 2 } else { 1 });
        exact.memory.write_word(CURRENT_OBJECT, root);
        let strategy = u32::from(exact.memory.read_word(root + FIELD_STRATEGY))
            | (u32::from(exact.memory.read_byte(root + FIELD_STRATEGY + 2)) << 16);
        exact.run_retail_oracle_routine(strategy, root).unwrap();
    }
    let actor = active_objects(&exact.memory)
        .into_iter()
        .find(|actor| exact.memory.read_word(actor + FIELD_PATH) == 0xFDC2)
        .unwrap();
    assert_eq!(exact.memory.read_word(actor + FIELD_SHAPE), 0xE194);
    assert_eq!(exact.memory.read_byte(actor + 0x2D), 1);
    assert_eq!(exact.memory.read_byte(actor + 0x2E), 16);
    exact.memory.write_word(ACTIVE_LIST, actor);
    exact.memory.write_word(actor, 0);
    exact.memory.write_word(actor + 2, 0);
    (exact, actor)
}

#[test]
fn complete_parent_path_matches_original_motion_animation_and_ordered_spawns() {
    let rom = retail();
    for schedule in 0..5 {
        for listener_case in 0..3 {
            let (mut exact, actor) = authored_craft(&rom);
            let mut native = OpeningSecondFlybyCraft::new();
            let mut child_flags = 0u16;
            let mut spawn_count = 0;
            for update in 0..600 {
                let listener = IntroScenePose {
                    position: match listener_case {
                        0 => Vector3::default(),
                        1 => Vector3 {
                            x: native.pose.position.x.wrapping_add(200),
                            y: -901,
                            z: native.pose.position.z,
                        },
                        2 => Vector3 {
                            x: native.pose.position.x.wrapping_add(1000),
                            y: 23000,
                            z: native.pose.position.z,
                        },
                        _ => unreachable!(),
                    },
                    rotation: Rotation {
                        yaw: Angle::from_units((update as u8).wrapping_mul(7)),
                        ..Default::default()
                    },
                };
                for (field, value) in [12, 14, 16].into_iter().zip([
                    listener.position.x,
                    listener.position.y,
                    listener.position.z,
                ]) {
                    exact.memory.write_word(VIEW + field, value as u16);
                }
                exact
                    .memory
                    .write_byte(VIEW + 21, listener.rotation.yaw.units());
                let (cue, encoded) = cue_at(schedule, update);
                exact.memory.write_byte(0x1D72, encoded);
                exact.memory.write_word(0x1D16, 0);
                let before = active_objects(&exact.memory);
                let strategy = u32::from(exact.memory.read_word(actor + FIELD_STRATEGY))
                    | (u32::from(exact.memory.read_byte(actor + FIELD_STRATEGY + 2)) << 16);
                exact.memory.write_word(CURRENT_OBJECT, actor);
                exact.run_retail_oracle_routine(strategy, actor).unwrap();
                let events = native.tick(cue);
                let context = format!(
                    "schedule={schedule} update={update} phase={:?} path={:04x}",
                    native.phase(),
                    exact.memory.read_word(actor + FIELD_PATH)
                );
                assert_eq!(
                    vector(&exact, actor, [12, 14, 16]),
                    native.pose.position,
                    "position {context}"
                );
                assert_eq!(
                    vector(&exact, actor, [50, 52, 54]),
                    native.velocity,
                    "velocity {context}"
                );
                assert_eq!(
                    [18, 20, 22].map(|field| exact.memory.read_byte(actor + field)),
                    [
                        native.pose.rotation.pitch.units(),
                        native.pose.rotation.yaw.units(),
                        native.pose.rotation.roll.units()
                    ],
                    "rotation {context}"
                );
                assert_eq!(
                    exact.memory.read_byte(actor + 24),
                    native.speed,
                    "speed {context}"
                );
                assert_eq!(
                    exact.memory.read_byte(actor + 11),
                    native.deceleration,
                    "approach {context}"
                );
                assert_eq!(
                    exact.memory.read_byte(actor + 0x1CCB),
                    native.animation_frame | if native.animation_enabled { 128 } else { 0 },
                    "animation {context}"
                );
                assert_eq!(
                    exact.memory.read_word(actor + FIELD_PATH),
                    source_path(native.phase()),
                    "path {context}"
                );
                let expected_audio: Vec<_> = events
                    .iter()
                    .filter_map(|event| {
                        let OpeningSecondFlybyEvent::Sound { sound, source } = event else {
                            return None;
                        };
                        use OpeningSecondFlybySound::*;
                        let mut value = match sound {
                            FlightBeat => 179,
                            TrailLeadIn => 189,
                            TrailAccent => 180,
                            WingDeparture => 190,
                            ExitBank => 130,
                            FinalBank => 188,
                        };
                        if *sound == FlightBeat {
                            let (volume, stereo) = OpeningBurstAudio {
                                sound: OpeningBurstSound::Burst,
                                source: source.position,
                            }
                            .spatial(listener);
                            value += match volume {
                                IntroExplosionVolume::Near => 0,
                                IntroExplosionVolume::Middle => 0x3000,
                                IntroExplosionVolume::Far => 0x6000,
                            };
                            value += match stereo {
                                StereoPosition::Center => 0,
                                StereoPosition::Left => 0x1000,
                                StereoPosition::Right => 0x2000,
                            };
                        }
                        Some(value)
                    })
                    .collect();
                let actual_audio: Vec<_> = (0..exact.memory.read_word(0x1D16))
                    .step_by(2)
                    .map(|index| exact.memory.read_word(0x1CF6 + index))
                    .collect();
                assert_eq!(
                    actual_audio, expected_audio,
                    "audio listener={listener_case} {context}"
                );
                let after = active_objects(&exact.memory);
                let mut spawned: Vec<_> = after
                    .into_iter()
                    .filter(|id| !before.contains(id))
                    .collect();
                // Each source allocation inserts after the head, so reverse the
                // new slice to recover ordered construction within this update.
                spawned.reverse();
                let spawn_events: Vec<_> = events
                    .iter()
                    .filter_map(|event| {
                        if let OpeningSecondFlybyEvent::Spawn(spawn) = event {
                            Some(spawn)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(spawned.len(), spawn_events.len(), "spawn count {context}");
                spawn_count += spawned.len();
                for (child, spawn) in spawned.into_iter().zip(spawn_events) {
                    assert_eq!(
                        exact.memory.read_word(child + FIELD_SHAPE),
                        0xBC9C + spawn.shape().catalog_index() as u16 * 28,
                        "spawn shape {context}"
                    );
                    let (path, parameter) = match spawn.child {
                        OpeningSecondFlybyChild::LinkedChain => (0xF831, 1),
                        OpeningSecondFlybyChild::EngineFlare => (0xFD58, 1),
                        OpeningSecondFlybyChild::Trail => (0xFF8D, 1),
                        OpeningSecondFlybyChild::CameraTarget => (0xFB2E, 1),
                        OpeningSecondFlybyChild::AttachedWing => (0xFF45, 1),
                        OpeningSecondFlybyChild::DepartingWing => (0xFF63, 19),
                    };
                    assert_eq!(exact.memory.read_word(child + FIELD_PATH), path);
                    assert_eq!(exact.memory.read_byte(child + 0x2E), parameter);
                    match spawn.placement {
                        OpeningSecondFlybySpawnPlacement::Independent(pose) => {
                            assert_eq!(
                                vector(&exact, child, [12, 14, 16]),
                                pose.position,
                                "independent spawn {context}"
                            );
                        }
                        OpeningSecondFlybySpawnPlacement::Attached(attachment) => {
                            assert_eq!(
                                vector(&exact, child, [0x1CCF, 0x1CD1, 0x1CD3]),
                                attachment.offset,
                                "attachment {context}"
                            );
                            assert_eq!(
                                [0x1CD5, 0x1CD6, 0x1CD7]
                                    .map(|field| exact.memory.read_byte(child + field)),
                                [
                                    attachment.rotation.pitch.units(),
                                    attachment.rotation.yaw.units(),
                                    attachment.rotation.roll.units()
                                ],
                                "attachment rotation {context}"
                            );
                        }
                    }
                    if let Some(group) = spawn.attachment_group() {
                        let encoded_group = match group {
                            OpeningSecondFlybyAttachmentGroup::Chain => 80,
                            OpeningSecondFlybyAttachmentGroup::Effects => 1,
                            OpeningSecondFlybyAttachmentGroup::Wing => 6,
                        };
                        assert_eq!(exact.memory.read_byte(child + 0x13), encoded_group);
                        assert_eq!(exact.memory.read_word(child + 6), actor);
                    }
                    if spawn.has_secondary_parent_link() {
                        assert_eq!(exact.memory.read_word(child + 0x1C), actor);
                    }
                }
                for event in events {
                    match event {
                        OpeningSecondFlybyEvent::InitializeChildControls => child_flags = 128,
                        OpeningSecondFlybyEvent::EnableChildPitchSettling => child_flags |= 8,
                        OpeningSecondFlybyEvent::SelectAsCameraTarget => {
                            assert_eq!(exact.memory.read_word(0x1DFF), actor)
                        }
                        _ => {}
                    }
                }
                assert_eq!(
                    exact.memory.read_word(0xD77D),
                    child_flags,
                    "child controls {context}"
                );
                assert_eq!(exact.memory.read_byte(actor + 37) & 8, 0);
            }
            if schedule < 2 {
                assert_eq!(native.phase(), OpeningSecondFlybyPhase::Holding);
                assert_eq!(spawn_count, 6);
            }
        }
    }
}

fn source_path(phase: OpeningSecondFlybyPhase) -> u16 {
    use OpeningSecondFlybyPhase::*;
    match phase {
        Initializing => unreachable!(),
        RockingOut { updates_left: 8 } => 0xFDE8,
        RockingOut { .. } => 0xFDEA,
        RockingBack { .. } => 0xFDF6,
        Maneuver { kind, .. } => {
            use OpeningSecondFlybyManeuver::*;
            match kind {
                PitchUp => 0xFE26,
                FirstPitchDown => 0xFE30,
                SecondPitchDown => 0xFE38,
                LastPitchDown => 0xFE42,
                TurnTowardTrail => 0xFE4B,
                LevelForDeparture => 0xFE8B,
                AimDeparture => 0xFE94,
                WingSeparationPause => 0xFEC3,
                BankAfterSeparation => 0xFECD,
                ExitRoll => 0xFED9,
                ExitTurn => 0xFEE8,
                SettleYaw => 0xFEF6,
                FinalApproach => 0xFF0C,
                FinalYawOscillation => 0xFF1D,
                FinalTurn => 0xFF30,
            }
        }
        AwaitingThirdCut => 0xFE66,
        AwaitingFourthCut => 0xFE77,
        BeforeWingSpawn { .. } => 0xFEA6,
        AfterWingSpawn { .. } => 0xFEB9,
        WingDeparturePause => 0xFEBF,
        AwaitingFinalCut => 0xFEFF,
        Holding => 0xFF43,
    }
}
