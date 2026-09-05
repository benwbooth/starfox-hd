//! Complete, naturally traversed later flyby: no child strategies are disabled
//! and no caller supplies guessed birth timing to the native scene.

use sf2_game::intro_attached_craft::{OpeningBurstAudio, OpeningBurstSound};
use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_chain::{OpeningChainControls, OpeningChainPart, OpeningChainPhase};
use sf2_game::intro_destruction::{
    IntroDestructionContext, IntroExplosionAppearance, IntroExplosionPhase, IntroExplosionVolume,
};
use sf2_game::intro_free_craft::IntroAuxiliaryEffect;
use sf2_game::intro_motion::IntroScenePose;
use sf2_game::intro_second_flyby_craft::{OpeningSecondFlybyEvent, OpeningSecondFlybySound};
use sf2_game::intro_second_flyby_scene::{
    OpeningSecondFlybyActor as Actor, OpeningSecondFlybyScene,
};
use sf2_game::object::{
    active_objects, allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY, FREE_LIST,
    PLAYER_ONE, SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, ObjectId, RandomState, Rotation, StereoPosition, Vector3};

const VIEW: u16 = 0x033F;
const AUX: u16 = 0x0140;
const STRATEGY: u32 = 0x7F7E1E;

fn retail() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("later flyby scene verification requires the user-owned retail SF2 ROM")
}

fn cue_at(schedule: u8, update: usize) -> (OpeningCameraCue, u8) {
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
        _ => unreachable!(),
    }
}

fn source_id(base: u16, id: ObjectId) -> u16 {
    base + id.index() as u16 * 63
}
fn vector(exact: &Game, actor: u16, fields: [u16; 3]) -> Vector3 {
    let [x, y, z] = fields.map(|field| exact.memory.read_word(actor + field) as i16);
    Vector3 { x, y, z }
}
fn pose(exact: &Game, actor: u16) -> IntroScenePose {
    IntroScenePose {
        position: vector(exact, actor, [12, 14, 16]),
        rotation: Rotation {
            pitch: Angle::from_units(exact.memory.read_byte(actor + 18)),
            yaw: Angle::from_units(exact.memory.read_byte(actor + 20)),
            roll: Angle::from_units(exact.memory.read_byte(actor + 22)),
        },
    }
}
fn write_position(exact: &mut Game, actor: u16, position: Vector3) {
    for (field, value) in [12, 14, 16]
        .into_iter()
        .zip([position.x, position.y, position.z])
    {
        exact.memory.write_word(actor + field, value as u16);
    }
}
fn encoded_controls(controls: OpeningChainControls) -> u16 {
    [
        (controls.suppress_initial_contact, 2),
        (controls.sort_override_on_reveal, 128),
        (controls.depart, 64),
        (controls.raise_depth_offset, 1024),
        (controls.settle_pitch, 8),
        (controls.bank_by_part, 4),
        (controls.level_pitch, 256),
    ]
    .into_iter()
    .filter_map(|(enabled, mask)| enabled.then_some(mask))
    .sum()
}

fn check_actor(exact: &Game, base: u16, id: ObjectId, native: &Actor, update: usize) {
    let actor = source_id(base, id);
    assert_eq!(
        exact.memory.read_word(actor + FIELD_SHAPE),
        0xBC9C + native.shape().catalog_index() as u16 * 28,
        "shape update={update} id={id:?}"
    );
    assert_eq!(
        vector(exact, actor, [12, 14, 16]),
        native.pose().position,
        "position update={update} id={id:?} native={native:?}"
    );
    if !matches!(native, Actor::Explosion(_)) {
        assert_eq!(
            pose(exact, actor).rotation,
            native.pose().rotation,
            "rotation update={update} id={id:?} native={native:?}"
        );
    }
    match native {
        Actor::Craft(craft) => {
            assert_eq!(vector(exact, actor, [50, 52, 54]), craft.velocity);
            assert_eq!(exact.memory.read_byte(actor + 24), craft.speed);
            assert_eq!(
                exact.memory.read_byte(actor + 0x1CCB),
                craft.animation_frame | if craft.animation_enabled { 128 } else { 0 }
            );
        }
        Actor::Chain(segment) => {
            assert_eq!(
                exact.memory.read_word(actor + 6),
                source_id(base, segment.parent())
            );
            assert_eq!(
                exact.memory.read_word(actor + 0x1C),
                source_id(base, segment.predecessor())
            );
            assert_eq!(exact.memory.read_word(actor + 0x1CD8), actor);
            assert_eq!(
                vector(exact, actor, [0x1CCF, 0x1CD1, 0x1CD3]),
                segment.local_offset
            );
            assert_eq!(vector(exact, actor, [50, 52, 54]), segment.velocity);
            assert_eq!(exact.memory.read_byte(actor + 0x2D), segment.health());
            assert_eq!(
                exact.memory.read_byte(actor + 0x21) & 1 != 0,
                segment.contact_disabled()
            );
            assert_eq!(
                exact.memory.read_byte(actor + 0x23) & 2 == 0,
                segment.is_visible()
            );
            assert_eq!(
                exact.memory.read_byte(actor + 9) & 1 != 0,
                segment.sort_override()
            );
            assert_eq!(exact.memory.read_word(actor + 0x1CC8), segment.depth_offset);
            assert_eq!(
                exact.memory.read_word(actor + FIELD_PATH),
                match segment.phase() {
                    OpeningChainPhase::HiddenUntilNextUpdate => 0xF868,
                    OpeningChainPhase::Following => 0xF881,
                    OpeningChainPhase::Departing { .. } => 0xF8CD,
                    _ => unreachable!(),
                }
            );
        }
        Actor::Flare(flare) => {
            assert_eq!(
                exact.memory.read_word(actor + 6),
                source_id(base, flare.parent())
            );
            assert_eq!(
                exact.memory.read_word(actor + 0x1CD8),
                source_id(base, flare.parent())
            );
            assert_eq!(exact.memory.read_word(actor + FIELD_STRATEGY), 0x9DDE);
            assert_eq!(
                exact.memory.read_byte(actor + 0x21) & 1 != 0,
                flare.contact_disabled()
            );
            assert_eq!(
                exact.memory.read_byte(actor + 9) & 1 != 0,
                flare.sort_override()
            );
        }
        Actor::Trail {
            parent,
            actor: trail,
        } => {
            assert_eq!(exact.memory.read_word(actor + 6), source_id(base, *parent));
            assert_eq!(
                vector(exact, actor, [0x1CCF, 0x1CD1, 0x1CD3]),
                trail.attachment.offset
            );
            assert_eq!(exact.memory.read_byte(actor + 0x1CEF), trail.depth_offset());
        }
        Actor::CameraTarget(target) => {
            assert_eq!(vector(exact, actor, [50, 52, 54]), target.velocity);
            assert_eq!(exact.memory.read_byte(actor + 24), target.speed);
        }
        Actor::AttachedWing(wing) => {
            assert_eq!(vector(exact, actor, [50, 52, 54]), wing.velocity);
            assert_eq!(
                exact.memory.read_word(actor + 6),
                wing.parent().map(|id| source_id(base, id)).unwrap_or(0)
            );
        }
        Actor::DepartingWing(wing) => {
            assert_eq!(vector(exact, actor, [50, 52, 54]), wing.velocity);
            assert_eq!(exact.memory.read_byte(actor + 24), wing.speed);
            assert_eq!(
                exact.memory.read_byte(actor + 0x23) & 2 == 0,
                wing.is_visible()
            );
        }
        Actor::ChainBurst(burst) => {
            assert_eq!(
                exact.memory.read_byte(actor + 0x1CCA) & 127,
                burst.color_frame
            );
            assert_eq!(exact.memory.read_byte(actor + 0x1CDA), burst.size_bias());
            assert_eq!(
                exact.memory.read_byte(actor + 0x20) & 32 != 0,
                burst.is_sprite()
            );
        }
        Actor::Explosion(effect) => {
            assert_eq!(
                exact.memory.read_byte(actor + 0x1CCA) & 127,
                effect.color_frame
            );
            match effect.appearance {
                IntroExplosionAppearance::Sprite { size_bias, .. } => {
                    assert_eq!(exact.memory.read_byte(actor + 0x1CDA), size_bias)
                }
                IntroExplosionAppearance::Companion { channels } => {
                    assert_eq!(
                        [19, 21, 23].map(|offset| exact.memory.read_byte(actor + offset)),
                        channels
                    );
                }
            }
            if let IntroExplosionPhase::Animating { age, limit } = effect.phase() {
                assert_eq!(exact.memory.read_byte(actor + 10), age);
                assert_eq!(exact.memory.read_byte(actor + 11), limit);
            }
        }
    }
}

fn sound_word(
    sound: OpeningSecondFlybySound,
    source: IntroScenePose,
    listener: IntroScenePose,
) -> u16 {
    use OpeningSecondFlybySound::*;
    let mut word = match sound {
        FlightBeat => 179,
        TrailLeadIn => 189,
        TrailAccent => 180,
        WingDeparture => 190,
        ExitBank => 130,
        FinalBank => 188,
    };
    if sound == FlightBeat {
        let (volume, stereo) = OpeningBurstAudio {
            sound: OpeningBurstSound::Burst,
            source: source.position,
        }
        .spatial(listener);
        word += volume_word(volume);
        word += match stereo {
            StereoPosition::Center => 0,
            StereoPosition::Left => 0x1000,
            StereoPosition::Right => 0x2000,
        };
    }
    word
}
fn volume_word(volume: IntroExplosionVolume) -> u16 {
    match volume {
        IntroExplosionVolume::Near => 0,
        IntroExplosionVolume::Middle => 0x3000,
        IntroExplosionVolume::Far => 0x6000,
    }
}

#[test]
fn complete_live_family_matches_original_traversal_publication_effect_birth_and_cleanup() {
    let rom = retail();
    for schedule in 0..3 {
        for departure in [false, true] {
            for scrolling in [false, true] {
                let mut exact = Game::new(rom.clone()).unwrap();
                let parent = allocate(&mut exact.memory, 0).unwrap();
                exact.memory.write_word(parent + FIELD_PATH, 0xFDC2);
                exact.memory.write_word(parent + FIELD_SHAPE, 0xE194);
                exact
                    .memory
                    .write_word(parent + FIELD_STRATEGY, STRATEGY as u16);
                exact
                    .memory
                    .write_byte(parent + FIELD_STRATEGY + 2, (STRATEGY >> 16) as u8);
                exact.memory.write_byte(parent + 0x2D, 1);
                exact.memory.write_byte(parent + 0x2E, 16);
                exact.memory.write_word(PLAYER_ONE, VIEW);
                exact.memory.write_word(SELECTED_OBJECT, VIEW);
                exact.memory.write_word(VIEW + FIELD_PATH, AUX);
                exact
                    .memory
                    .write_byte(0x1AA6, if scrolling { 0 } else { 2 });
                let seed = if scrolling { [1, 255, 0, 128] } else { [0; 4] };
                for (index, byte) in seed.into_iter().enumerate() {
                    exact.memory.write_byte(0xE0 + index as u16, byte);
                }
                let mut native = OpeningSecondFlybyScene::new();
                let mut random = RandomState::new(seed);
                let mut auxiliary = IntroAuxiliaryEffect::default();
                let mut saw_effects = false;
                let mut saw_bursts = false;
                for update in 0..550 {
                    let (cue, encoded) = cue_at(schedule, update);
                    exact.memory.write_byte(0x1D72, encoded);
                    if departure {
                        native.chain_controls.depart = update >= 190;
                        native.chain_controls.bank_by_part = update % 3 == 1;
                        native.chain_controls.level_pitch = update % 7 == 3;
                        native.chain_controls.raise_depth_offset = update >= 75;
                        exact
                            .memory
                            .write_word(0xD77D, encoded_controls(native.chain_controls));
                    }
                    let listener = IntroScenePose {
                        position: Vector3 {
                            x: (update as i16).wrapping_mul(101),
                            y: -139,
                            z: -711,
                        },
                        rotation: Rotation {
                            yaw: Angle::from_units((update as u8).wrapping_mul(7)),
                            ..Default::default()
                        },
                    };
                    write_position(&mut exact, VIEW, listener.position);
                    exact
                        .memory
                        .write_byte(VIEW + 21, listener.rotation.yaw.units());
                    let context = IntroDestructionContext {
                        primary_listener: listener.position,
                        secondary_listener: scrolling.then_some(Vector3 {
                            x: -12500,
                            y: 13,
                            z: 27900,
                        }),
                        compensate_scroll: scrolling,
                        scroll: Vector3 {
                            x: update as i16 * 7,
                            y: 0,
                            z: update as i16 * -3,
                        },
                        ..Default::default()
                    };
                    if let Some(position) = context.secondary_listener {
                        write_position(&mut exact, 0x037E, position);
                    }
                    if [15, 16, 190].contains(&update) {
                        let first = native.actors().find_map(|(id, actor)| matches!(actor,
                            Actor::Chain(segment) if segment.part() == OpeningChainPart::First && segment.phase() == OpeningChainPhase::Following).then_some(id));
                        if let Some(id) = first {
                            let actor = source_id(parent, id);
                            exact.memory.write_byte(actor + 0x2D, 0);
                            let flags = exact.memory.read_byte(actor + 0x31) | 4;
                            exact.memory.write_byte(actor + 0x31, flags);
                            native.set_chain_health_at_strategy_entry(OpeningChainPart::First, 0);
                        }
                    }
                    exact
                        .memory
                        .write_byte(AUX + 0x6AA0, if scrolling { 16 } else { 0 });
                    exact.memory.write_word(0x1E1C, context.scroll.x as u16);
                    exact.memory.write_word(0x1E20, context.scroll.z as u16);
                    exact.memory.write_word(0x1D16, 0);
                    exact.memory.write_word(CURRENT_OBJECT, parent);
                    exact.run_retail_oracle_routine(0x7F34E7, parent).unwrap();
                    exact.run_retail_oracle_routine(0x7F354A, parent).unwrap();
                    let events = native
                        .tick(cue, &mut random, &mut auxiliary, &context)
                        .unwrap();
                    for id in &events.retired {
                        assert_eq!(exact.memory.read_byte(source_id(parent, *id) + 0x25) & 8, 8);
                    }
                    exact.run_retail_oracle_routine(0x7F402D, parent).unwrap();
                    let source = active_objects(&exact.memory);
                    let expected: Vec<_> = native
                        .actors()
                        .map(|(id, _)| source_id(parent, id))
                        .collect();
                    assert_eq!(
                        source, expected,
                        "active order schedule={schedule} departure={departure} update={update}"
                    );
                    for (id, actor) in native.actors() {
                        check_actor(&exact, parent, id, actor, update);
                        saw_effects |= matches!(actor, Actor::Explosion(_));
                        saw_bursts |= matches!(actor, Actor::ChainBurst(_));
                    }
                    assert_eq!(
                        exact.memory.read_word(0xD77D),
                        encoded_controls(native.chain_controls)
                    );
                    assert_eq!(
                        random.bytes(),
                        std::array::from_fn(|i| exact.memory.read_byte(0xE0 + i as u16))
                    );
                    if let Some(id) = native.camera_target() {
                        assert_eq!(exact.memory.read_word(0x1DFF), source_id(parent, id));
                    }
                    let mut free = exact.memory.read_word(FREE_LIST);
                    let mut available = 0;
                    while free != 0 {
                        available += 1;
                        free = exact.memory.read_word(free);
                    }
                    assert_eq!(native.available_slots(), available);
                    let mut expected_audio: Vec<u16> = events
                        .craft_events
                        .iter()
                        .filter_map(|event| match event {
                            OpeningSecondFlybyEvent::Sound { sound, source } => {
                                Some(sound_word(*sound, *source, listener))
                            }
                            _ => None,
                        })
                        .collect();
                    expected_audio.extend(
                        events
                            .explosion_audio
                            .into_iter()
                            .map(|volume| 0x70 + volume_word(volume)),
                    );
                    let source_audio: Vec<_> = (0..exact.memory.read_word(0x1D16))
                        .step_by(2)
                        .map(|i| exact.memory.read_word(0x1CF6 + i))
                        .collect();
                    assert_eq!(source_audio, expected_audio, "audio update={update}");
                    assert_eq!(
                        vector(&exact, AUX, [0x6A92, 0x6A94, 0x6A96]),
                        auxiliary.origin
                    );
                    assert_eq!(
                        exact.memory.read_word(AUX + 0x6A98),
                        auxiliary.owner.map(|id| source_id(parent, id)).unwrap_or(0)
                    );
                    assert_eq!(
                        exact.memory.read_byte(AUX + 0x6A8C),
                        if auxiliary.frozen { 128 } else { 0 }
                            | if auxiliary.tracking { 64 } else { 0 }
                    );
                    assert_eq!(
                        [0x6A8D, 0x6A8E, 0x6A8F].map(|field| exact.memory.read_byte(AUX + field)),
                        auxiliary.axis_modes
                    );
                    assert_eq!(
                        [0x6C2A, 0x6C2B, 0x6C2C].map(|field| exact.memory.read_byte(AUX + field)),
                        auxiliary.axis_controls
                    );
                    for (field, value) in [
                        (0x6A90, auxiliary.range as u16),
                        (0x6C1C, auxiliary.transition_mode),
                        (0x6C24, auxiliary.limit),
                        (0x6C26, auxiliary.remaining as u16),
                    ] {
                        assert_eq!(exact.memory.read_word(AUX + field), value);
                    }
                    assert_eq!(exact.memory.read_byte(AUX + 0x6C29), auxiliary.target_axis);
                    assert_eq!(
                        exact.memory.read_byte(AUX + 0x6C28),
                        auxiliary.target_control
                    );
                }
                assert_eq!(saw_effects, schedule != 2);
                assert_eq!(saw_bursts, departure);
                assert_eq!(
                    native
                        .actors()
                        .filter(|(_, actor)| matches!(actor, Actor::Chain(_)))
                        .count(),
                    if departure { 0 } else { 9 }
                );
                assert_eq!(
                    native
                        .actors()
                        .filter(|(_, actor)| matches!(actor, Actor::Flare(_)))
                        .count(),
                    usize::from(schedule != 2)
                );
            }
        }
    }
}
