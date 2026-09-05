//! Source-created wings, real parent publication and actor-list updates.

use sf2_game::intro_destruction::{
    IntroDestructionContext, IntroExplosionAppearance, IntroExplosionBirthTiming,
    IntroExplosionPhase, IntroExplosionVolume,
};
use sf2_game::intro_free_craft::IntroAuxiliaryEffect;
use sf2_game::intro_motion::{IntroAttachment, IntroScenePose};
use sf2_game::intro_second_flyby_wings::{
    OpeningAttachedWing, OpeningAttachedWingPhase, OpeningDepartingWing, OpeningDepartingWingPhase,
    OpeningWing, OpeningWingSequence,
};
use sf2_game::object::{
    active_objects, allocate, ACTIVE_LIST, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY,
    PLAYER_ONE, SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{
    Angle, Behavior, Object, ObjectId, ObjectKind, ObjectStore, Rotation, ShapeId, Vector3,
};

const STRATEGY: u32 = 0x7F7E1E;
const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const VIEW: u16 = 0x033F;
const AUX: u16 = 0x0140;
const POSITION: [u16; 3] = [12, 14, 16];
const ROTATION: [u16; 3] = [18, 20, 22];
const VELOCITY: [u16; 3] = [50, 52, 54];

fn retail() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("wing verification requires the user-owned retail SF2 ROM")
}

#[test]
fn both_wing_lifetimes_match_original_through_final_effect_removal_and_birth_timing() {
    let rom = retail();
    for independent in [false, true] {
        for birth_now in [false, true] {
            for scrolling in [false, true] {
                let (mut exact, parent, actor) =
                    authored_wing(&rom, independent, parent_pose(11, true));
                let (id, parent_id) = ids();
                let wing = if independent {
                    OpeningWing::Departing(OpeningDepartingWing::new(id, parent_pose(11, true)))
                } else {
                    OpeningWing::Attached(OpeningAttachedWing::new(
                        id,
                        parent_id,
                        IntroAttachment {
                            offset: Vector3 { x: 0, y: 20, z: 50 },
                            rotation: Rotation {
                                pitch: Angle::ZERO,
                                yaw: Angle::from_units(48),
                                roll: Angle::from_units(236),
                            },
                        },
                    ))
                };
                let mut native = OpeningWingSequence::new(wing);
                let mut auxiliary = IntroAuxiliaryEffect::default();
                let mut detached = independent;
                let mut head = parent;
                let birth_timing = if birth_now {
                    IntroExplosionBirthTiming::ThisUpdate
                } else {
                    IntroExplosionBirthTiming::NextUpdate
                };
                for update in 0..100 {
                    // With the wing as list head, newly allocated effects run
                    // immediately. Behind the live parent they miss traversal.
                    // An attached wing is isolated only after its real unlink.
                    if birth_now && detached && head == parent {
                        exact.memory.write_word(ACTIVE_LIST, actor);
                        exact.memory.write_word(actor, 0);
                        exact.memory.write_word(actor + 2, 0);
                        head = actor;
                    }
                    let pose = parent_pose(update, true);
                    write_pose(&mut exact, parent, pose);
                    native.publish_from_parent(parent_id, pose);
                    let mut available = 0;
                    let mut free = exact.memory.read_word(sf2_game::object::FREE_LIST);
                    while free != 0 {
                        available += 1;
                        free = exact.memory.read_word(free);
                    }
                    let context = IntroDestructionContext {
                        available_slots: available,
                        primary_listener: Vector3 {
                            x: 1300,
                            y: 219,
                            z: -700,
                        },
                        compensate_scroll: scrolling,
                        scroll: Vector3 {
                            x: update as i16 * 7,
                            y: 0,
                            z: update as i16 * -3,
                        },
                        ..Default::default()
                    };
                    write_pose(
                        &mut exact,
                        VIEW,
                        IntroScenePose {
                            position: context.primary_listener,
                            ..Default::default()
                        },
                    );
                    exact
                        .memory
                        .write_byte(AUX + 0x6AA0, if scrolling { 16 } else { 0 });
                    exact.memory.write_word(0x1E1C, context.scroll.x as u16);
                    exact.memory.write_word(0x1E20, context.scroll.z as u16);
                    exact.memory.write_word(0x1D16, 0);
                    exact.memory.write_word(CURRENT_OBJECT, head);
                    exact.run_retail_oracle_routine(UPDATE, head).unwrap();
                    exact.run_retail_oracle_routine(RESUME, head).unwrap();
                    let events = native.tick(&mut auxiliary, &context, birth_timing).unwrap();
                    detached |= events.detached;
                    let expected_audio: Vec<u16> = events
                        .explosion_audio
                        .iter()
                        .map(|volume| match volume {
                            IntroExplosionVolume::Near => 0x70,
                            IntroExplosionVolume::Middle => 0x3070,
                            IntroExplosionVolume::Far => 0x6070,
                        })
                        .collect();
                    let actual_audio: Vec<u16> = (0..exact.memory.read_word(0x1D16))
                        .step_by(2)
                        .map(|offset| exact.memory.read_word(0x1CF6 + offset))
                        .collect();
                    assert_eq!(
                        actual_audio, expected_audio,
                        "audio independent={independent} birth_now={birth_now} update={update}"
                    );
                    if !native.craft_has_retired() {
                        assert_pose(&exact, actor, native.wing.pose(), update);
                        assert_eq!(
                            exact.memory.read_byte(actor + 0x23) & 2 == 0,
                            native.craft_is_visible()
                        );
                    }
                    exact.run_retail_oracle_routine(0x7F402D, head).unwrap();
                    if native.craft_has_retired() {
                        let mut expected = Vec::new();
                        for effect in native.effects().filter(|effect| !effect.is_finished()) {
                            let (age, limit) = match effect.phase() {
                                IntroExplosionPhase::Animating { age, limit } => (age, limit),
                                IntroExplosionPhase::AwaitingDestruction => (1, 2),
                                IntroExplosionPhase::Finished => unreachable!(),
                            };
                            let (style, size_bias, channels) = match effect.appearance {
                                IntroExplosionAppearance::Sprite { size_bias, .. } => {
                                    (0, size_bias, [0, 0, 0])
                                }
                                IntroExplosionAppearance::Companion { channels } => {
                                    (16, 0, channels)
                                }
                            };
                            expected.push((
                                effect.position.x,
                                effect.position.y,
                                effect.position.z,
                                effect.shape().catalog_index() as u16,
                                effect.color_frame,
                                age,
                                limit,
                                style,
                                size_bias,
                                channels,
                            ));
                        }
                        let mut actual = Vec::new();
                        for effect in active_objects(&exact.memory)
                            .into_iter()
                            .filter(|id| *id != parent)
                        {
                            let position = vector(&exact, effect, POSITION);
                            let style = exact.memory.read_byte(effect + 0x20) & 16;
                            let channels = if style != 0 {
                                [19, 21, 23].map(|field| exact.memory.read_byte(effect + field))
                            } else {
                                [0, 0, 0]
                            };
                            actual.push((
                                position.x,
                                position.y,
                                position.z,
                                (exact.memory.read_word(effect + FIELD_SHAPE) - 0xBC9C) / 28,
                                exact.memory.read_byte(effect + 0x1CCA) & 127,
                                exact.memory.read_byte(effect + 10),
                                exact.memory.read_byte(effect + 11),
                                style,
                                if style == 0 {
                                    exact.memory.read_byte(effect + 0x1CDA)
                                } else {
                                    0
                                },
                                channels,
                            ));
                        }
                        actual.sort_unstable();
                        expected.sort_unstable();
                        assert_eq!(actual,expected,"effects independent={independent} birth_now={birth_now} scrolling={scrolling} update={update}");
                    }
                    if native.is_finished() {
                        break;
                    }
                }
                assert!(native.is_finished());
                let ended = native.clone();
                assert_eq!(
                    native
                        .tick(
                            &mut auxiliary,
                            &IntroDestructionContext::default(),
                            birth_timing
                        )
                        .unwrap(),
                    Default::default()
                );
                assert_eq!(native, ended);
            }
        }
    }
}

fn ids() -> (ObjectId, ObjectId) {
    let mut objects = ObjectStore::new();
    let mut allocate = || {
        objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::from_catalog_index(89),
                Behavior::Effect,
            ))
            .unwrap()
    };
    (allocate(), allocate())
}

fn write_pose(exact: &mut Game, actor: u16, pose: IntroScenePose) {
    for (field, value) in
        POSITION
            .into_iter()
            .zip([pose.position.x, pose.position.y, pose.position.z])
    {
        exact.memory.write_word(actor + field, value as u16);
    }
    for (field, angle) in
        ROTATION
            .into_iter()
            .zip([pose.rotation.pitch, pose.rotation.yaw, pose.rotation.roll])
    {
        exact.memory.write_byte(actor + field, angle.units());
    }
}

fn vector(exact: &Game, actor: u16, fields: [u16; 3]) -> Vector3 {
    let [x, y, z] = fields.map(|field| exact.memory.read_word(actor + field) as i16);
    Vector3 { x, y, z }
}

fn assert_pose(exact: &Game, actor: u16, pose: IntroScenePose, update: usize) {
    assert_eq!(
        vector(exact, actor, POSITION),
        pose.position,
        "position update={update}"
    );
    assert_eq!(
        ROTATION.map(|field| exact.memory.read_byte(actor + field)),
        [
            pose.rotation.pitch.units(),
            pose.rotation.yaw.units(),
            pose.rotation.roll.units()
        ],
        "rotation update={update}"
    );
}

fn authored_wing(rom: &[u8], independent: bool, pose: IntroScenePose) -> (Game, u16, u16) {
    let mut exact = Game::new(rom.to_vec()).unwrap();
    let parent = allocate(&mut exact.memory, 0).unwrap();
    exact.memory.write_word(
        parent + FIELD_PATH,
        if independent { 0xFEC4 } else { 0xFEA8 },
    );
    exact
        .memory
        .write_word(parent + FIELD_STRATEGY, STRATEGY as u16);
    exact
        .memory
        .write_byte(parent + FIELD_STRATEGY + 2, (STRATEGY >> 16) as u8);
    exact
        .memory
        .write_word(parent + FIELD_SHAPE, 0xBC9C + 338 * 28);
    exact.memory.write_byte(parent + 0x2D, 1);
    exact.memory.write_word(PLAYER_ONE, VIEW);
    exact.memory.write_word(SELECTED_OBJECT, VIEW);
    exact.memory.write_word(VIEW + FIELD_PATH, AUX);
    exact.memory.write_byte(0x1AA6, 2);
    write_pose(&mut exact, parent, pose);
    exact.memory.write_word(CURRENT_OBJECT, parent);
    exact.run_retail_oracle_routine(STRATEGY, parent).unwrap();
    let path = if independent { 0xFF63 } else { 0xFF45 };
    let actor = active_objects(&exact.memory)
        .into_iter()
        .find(|id| exact.memory.read_word(id + FIELD_PATH) == path)
        .unwrap();
    assert_eq!(
        exact.memory.read_word(actor + FIELD_SHAPE),
        0xBC9C + 89 * 28
    );
    assert_eq!(exact.memory.read_byte(actor + 0x2D), 1);
    assert_eq!(
        exact.memory.read_byte(actor + 0x2E),
        if independent { 19 } else { 1 }
    );
    if independent {
        // Both position and rotation belong to the pre-command parent pose.
        assert_pose(&exact, actor, pose, 0);
    } else {
        assert_eq!(exact.memory.read_word(actor + 6), parent);
        assert_eq!(exact.memory.read_word(actor + 0x1CD8), parent);
        assert_eq!(exact.memory.read_byte(actor + 19), 6);
    }
    // Keep the genuine parent/child allocation order and attachment lists,
    // but use the parent's authored hold so its other child families do not run.
    exact.memory.write_word(parent + FIELD_PATH, 0xFF43);
    (exact, parent, actor)
}

fn parent_pose(update: usize, varied: bool) -> IntroScenePose {
    IntroScenePose {
        position: if varied {
            Vector3 {
                x: i16::MAX.wrapping_add(update as i16 * 71),
                y: i16::MIN.wrapping_sub(update as i16 * 39),
                z: -149,
            }
        } else {
            Vector3::default()
        },
        rotation: if varied {
            Rotation {
                pitch: Angle::from_units((update as u8).wrapping_mul(7)),
                yaw: Angle::from_units((update as u8).wrapping_mul(13)),
                roll: Angle::from_units((update as u8).wrapping_mul(29)),
            }
        } else {
            Rotation::default()
        },
    }
}

fn seed_auxiliary(
    exact: &mut Game,
    actor: u16,
    id: ObjectId,
    frozen: bool,
    owned: bool,
) -> IntroAuxiliaryEffect {
    let effect = IntroAuxiliaryEffect {
        frozen,
        tracking: true,
        owner: owned.then_some(id),
        ..Default::default()
    };
    exact
        .memory
        .write_byte(AUX + 0x6A8C, 64 | if frozen { 128 } else { 0 });
    exact
        .memory
        .write_word(AUX + 0x6A98, if owned { actor } else { VIEW });
    effect
}

fn assert_auxiliary(exact: &Game, actor: u16, id: ObjectId, effect: IntroAuxiliaryEffect) {
    assert_eq!(vector(exact, AUX, [0x6A92, 0x6A94, 0x6A96]), effect.origin);
    assert_eq!(
        exact.memory.read_word(AUX + 0x6A98),
        if effect.owner == Some(id) {
            actor
        } else {
            VIEW
        }
    );
    assert_eq!(
        exact.memory.read_byte(AUX + 0x6A8C),
        if effect.frozen { 128 } else { 0 } | if effect.tracking { 64 } else { 0 }
    );
    for (field, value) in [0x6A8D, 0x6A8E, 0x6A8F].into_iter().zip(effect.axis_modes) {
        assert_eq!(exact.memory.read_byte(AUX + field), value);
    }
    for (field, value) in [0x6C2A, 0x6C2B, 0x6C2C]
        .into_iter()
        .zip(effect.axis_controls)
    {
        assert_eq!(exact.memory.read_byte(AUX + field), value);
    }
    for (field, value) in [
        (0x6A90, effect.range as u16),
        (0x6C1C, effect.transition_mode),
        (0x6C24, effect.limit),
        (0x6C26, effect.remaining as u16),
    ] {
        assert_eq!(exact.memory.read_word(AUX + field), value);
    }
    assert_eq!(exact.memory.read_byte(AUX + 0x6C29), effect.target_axis);
    assert_eq!(exact.memory.read_byte(AUX + 0x6C28), effect.target_control);
}

#[test]
fn attached_wing_matches_publication_unlink_drift_rotation_and_destruction_boundary() {
    let rom = retail();
    for varied in [false, true] {
        for frozen in [false, true] {
            for owned in [false, true] {
                let (mut exact, parent, actor) =
                    authored_wing(&rom, false, parent_pose(11, varied));
                let (id, parent_id) = ids();
                let attachment = IntroAttachment {
                    offset: Vector3 { x: 0, y: 20, z: 50 },
                    rotation: Rotation {
                        pitch: Angle::ZERO,
                        yaw: Angle::from_units(48),
                        roll: Angle::from_units(236),
                    },
                };
                let mut native = OpeningAttachedWing::new(id, parent_id, attachment);
                let mut auxiliary = seed_auxiliary(&mut exact, actor, id, frozen, owned);
                let mut detach_count = 0;
                for update in 0..100 {
                    let pose = parent_pose(update, varied);
                    write_pose(&mut exact, parent, pose);
                    native.publish_from_parent(parent_id, pose);
                    exact.memory.write_word(CURRENT_OBJECT, parent);
                    exact.run_retail_oracle_routine(UPDATE, parent).unwrap();
                    exact.run_retail_oracle_routine(RESUME, parent).unwrap();
                    let event = native.tick(&mut auxiliary);
                    detach_count += usize::from(event.detached);
                    assert_pose(&exact, actor, native.pose, update);
                    assert_eq!(
                        vector(&exact, actor, VELOCITY),
                        native.velocity,
                        "velocity update={update}"
                    );
                    assert_eq!(
                        vector(&exact, actor, [0x1CCF, 0x1CD1, 0x1CD3]),
                        native.attachment.offset
                    );
                    assert_eq!(
                        exact.memory.read_word(actor + 6),
                        if native.parent().is_some() { parent } else { 0 }
                    );
                    assert_eq!(
                        exact.memory.read_byte(actor + 19),
                        if native.parent().is_some() { 6 } else { 0 }
                    );
                    assert_eq!(
                        exact.memory.read_byte(actor + 0x23) & 4 != 0,
                        native.parent().is_some()
                    );
                    assert_eq!(
                        exact.memory.read_byte(actor + 0x25) & 1 != 0,
                        native.parent().is_some()
                    );
                    assert_eq!(exact.memory.read_byte(actor + 24), 0);
                    let path = match native.phase() {
                        OpeningAttachedWingPhase::Waiting { .. } => 0xFF46,
                        OpeningAttachedWingPhase::Spinning { .. } => 0xFF54,
                        OpeningAttachedWingPhase::AwaitingDestruction => 0xFF62,
                    };
                    assert_eq!(
                        exact.memory.read_word(actor + FIELD_PATH),
                        path,
                        "path update={update}"
                    );
                    assert_eq!(
                        exact.memory.read_byte(actor + 0x2D) == 0,
                        event.request_destruction
                    );
                    assert_eq!(exact.memory.read_byte(actor + 0x25) & 8, 0);
                    assert_eq!(exact.memory.read_byte(actor + 0x23) & 2, 0);
                    assert_auxiliary(&exact, actor, id, auxiliary);
                    if event.request_destruction {
                        assert_eq!(detach_count, 1);
                        assert_eq!(update, 44);
                        exact.run_retail_oracle_routine(UPDATE, parent).unwrap();
                        exact.run_retail_oracle_routine(RESUME, parent).unwrap();
                        assert_eq!(
                            exact.memory.read_byte(actor + 0x25) & 8,
                            8,
                            "common destruction runs next update"
                        );
                        break;
                    }
                }
                assert_eq!(
                    native.phase(),
                    OpeningAttachedWingPhase::AwaitingDestruction
                );
            }
        }
    }
}

#[test]
fn independent_wing_matches_inherited_pose_authored_turns_visibility_and_auxiliary_handoff() {
    let rom = retail();
    for varied in [false, true] {
        for frozen in [false, true] {
            for owned in [false, true] {
                let inherited = parent_pose(11, varied);
                let (mut exact, parent, actor) = authored_wing(&rom, true, inherited);
                let (id, _) = ids();
                let mut native = OpeningDepartingWing::new(id, inherited);
                let mut auxiliary = seed_auxiliary(&mut exact, actor, id, frozen, owned);
                for update in 0..100 {
                    exact.memory.write_word(CURRENT_OBJECT, parent);
                    exact.run_retail_oracle_routine(UPDATE, parent).unwrap();
                    exact.run_retail_oracle_routine(RESUME, parent).unwrap();
                    let event = native.tick(&mut auxiliary);
                    assert_pose(&exact, actor, native.pose, update);
                    assert_eq!(
                        vector(&exact, actor, VELOCITY),
                        native.velocity,
                        "velocity update={update}"
                    );
                    assert_eq!(exact.memory.read_byte(actor + 24), native.speed);
                    assert_eq!(
                        exact.memory.read_byte(actor + 0x1CCC) != 0,
                        native.trail_enabled
                    );
                    assert_eq!(
                        exact.memory.read_byte(actor + 0x23) & 2 == 0,
                        native.is_visible()
                    );
                    let path = match native.phase() {
                        OpeningDepartingWingPhase::Initializing => unreachable!(),
                        OpeningDepartingWingPhase::HiddenTurn { .. } => 0xFF70,
                        OpeningDepartingWingPhase::VisibleTurn { .. } => 0xFF76,
                        OpeningDepartingWingPhase::Rolling { .. } => 0xFF83,
                        OpeningDepartingWingPhase::AwaitingDestruction => 0xFF8C,
                    };
                    assert_eq!(
                        exact.memory.read_word(actor + FIELD_PATH),
                        path,
                        "path update={update}"
                    );
                    assert_eq!(
                        exact.memory.read_byte(actor + 0x2D) == 0,
                        event.request_destruction
                    );
                    assert_eq!(exact.memory.read_byte(actor + 0x25) & 8, 0);
                    assert_auxiliary(&exact, actor, id, auxiliary);
                    if event.request_destruction {
                        assert_eq!(update, 52);
                        exact.run_retail_oracle_routine(UPDATE, parent).unwrap();
                        exact.run_retail_oracle_routine(RESUME, parent).unwrap();
                        assert_eq!(
                            exact.memory.read_byte(actor + 0x25) & 8,
                            8,
                            "common destruction runs next update"
                        );
                        break;
                    }
                }
                assert_eq!(
                    native.phase(),
                    OpeningDepartingWingPhase::AwaitingDestruction
                );
            }
        }
    }
}
