//! Complete attached camera-target path, driven by the original actor list.
//! Parent pose updates precede child logic; native code uses typed scene state.

use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_motion::IntroScenePose;
use sf2_game::intro_target::{OpeningCameraTarget, OpeningTargetPhase};
use sf2_game::object::{
    allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY, PLAYER_ONE,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, Behavior, Object, ObjectKind, ObjectStore, Rotation, ShapeId, Vector3};

const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const INITIAL_STRATEGY: u32 = 0x7F7E1E;
const HOLD_PATH: u16 = 0xFBB0;
const TARGET_PATH: u16 = 0xFAB2;
const VIEW: u16 = 0x033F;
const SELECTED_AUX: u16 = 0x0140;
const POSITION: [u16; 3] = [0x0C, 0x0E, 0x10];
const VELOCITY: [u16; 3] = [0x32, 0x34, 0x36];
const ROTATION: [u16; 3] = [0x12, 0x14, 0x16];
const OFFSET: [u16; 3] = [0x1CCF, 0x1CD1, 0x1CD3];
const LOCAL_ROTATION: [u16; 3] = [0x1CD5, 0x1CD6, 0x1CD7];
const SCENE_CUE: u16 = 0x1D72;
const ROTATION_TARGET: u16 = 0x1DFF;
const AIM_LINK: u16 = 0x1CE4;
const PARENT_LINK: u16 = 6;
const CHILD_LINK: u16 = 0x29;
const SHAPE_BASE: u16 = 0xBC9C;
const SHAPE_STRIDE: u16 = 28;
const MAX_UPDATES: usize = 240;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("target verification requires the user-owned retail SF2 ROM")
}

fn native_actor(shape: usize) -> Object {
    Object::new(
        ObjectKind::Effect,
        ShapeId::from_catalog_index(shape as u16),
        Behavior::Effect,
    )
}

fn write_vector(game: &mut Game, object: u16, fields: [u16; 3], value: Vector3) {
    for (field, value) in fields.into_iter().zip([value.x, value.y, value.z]) {
        game.memory.write_word(object + field, value as u16);
    }
}

fn assert_vector(game: &Game, object: u16, fields: [u16; 3], value: Vector3, update: usize) {
    for (field, expected) in fields.into_iter().zip([value.x, value.y, value.z]) {
        assert_eq!(
            game.memory.read_word(object + field) as i16,
            expected,
            "update={update} object={object} field={field}"
        );
    }
}

fn setup_path(game: &mut Game, object: u16, path: u16) {
    game.memory.write_word(object + FIELD_PATH, path);
    game.memory
        .write_word(object + FIELD_STRATEGY, INITIAL_STRATEGY as u16);
    game.memory
        .write_byte(object + FIELD_STRATEGY + 2, (INITIAL_STRATEGY >> 16) as u8);
    game.memory.write_byte(object + 0x2D, 1);
}

#[test]
fn target_lifetime_matches_retail_attachments_searches_and_cues() {
    let rom = retail();
    for opening_end in [0, 130] {
        for first_cut_end in [0, 200] {
            for candidates in [0, 1, 2, 3] {
                for rotating in [false, true] {
                    let mut actors = ObjectStore::new();
                    let parent_id = actors.allocate(native_actor(0)).unwrap();
                    let target_id = actors.allocate(native_actor(0)).unwrap();
                    let mut target = OpeningCameraTarget::new(target_id);
                    let mut exact = Game::new(rom.clone()).unwrap();
                    let parent = allocate(&mut exact.memory, 0).unwrap();
                    let object = allocate(&mut exact.memory, parent).unwrap();
                    let mut actor_mapping = vec![(parent_id, parent), (target_id, object)];
                    setup_path(&mut exact, parent, HOLD_PATH);
                    setup_path(&mut exact, object, TARGET_PATH);
                    exact.memory.write_word(parent + CHILD_LINK, object);
                    exact.memory.write_byte(parent + 0x23, 0x10);
                    exact.memory.write_word(object + PARENT_LINK, parent);
                    exact.memory.write_byte(object + 0x23, 4);
                    exact.memory.write_byte(object + 0x25, 1);
                    write_vector(&mut exact, object, OFFSET, target.attachment.offset);
                    exact.memory.write_word(PLAYER_ONE, VIEW);
                    exact.memory.write_word(VIEW + FIELD_PATH, SELECTED_AUX);
                    if candidates != 0 {
                        for shape in [64, 338, 64, 338] {
                            let id = actors.allocate(native_actor(shape)).unwrap();
                            let exact_id = allocate(&mut exact.memory, object).unwrap();
                            exact.memory.write_word(
                                exact_id + FIELD_SHAPE,
                                SHAPE_BASE + shape as u16 * SHAPE_STRIDE,
                            );
                            // Static scene inputs do not execute their own strategy.
                            exact.memory.write_byte(exact_id + 0x2D, 1);
                            actor_mapping.push((id, exact_id));
                        }
                    }
                    for update in 0..MAX_UPDATES {
                        let cue = if update < opening_end {
                            OpeningCameraCue::Opening
                        } else if update < first_cut_end {
                            OpeningCameraCue::FirstCut
                        } else {
                            OpeningCameraCue::SecondCut
                        };
                        let source_cue = match cue {
                            OpeningCameraCue::Opening => 1,
                            OpeningCameraCue::FirstCut => 2,
                            OpeningCameraCue::SecondCut => 3,
                            _ => unreachable!(),
                        };
                        let parent_pose = IntroScenePose {
                            position: Vector3 {
                                x: i16::MAX.wrapping_add(update as i16),
                                y: -701,
                                z: -1091,
                            },
                            rotation: if rotating {
                                Rotation {
                                    pitch: Angle::from_units(update as u8),
                                    yaw: Angle::from_units((update as u8).wrapping_mul(3)),
                                    roll: Angle::from_units((update as u8).wrapping_mul(7)),
                                }
                            } else {
                                Rotation::default()
                            },
                        };
                        for (index, &(id, exact_id)) in actor_mapping.iter().enumerate().skip(2) {
                            let lateral = match candidates {
                                1 => 900,
                                2 => (index as i16 * 311) - 1200,
                                3 => 14_000,
                                _ => unreachable!(),
                            };
                            let position = Vector3 {
                                x: parent_pose.position.x.wrapping_add(lateral),
                                y: parent_pose
                                    .position
                                    .y
                                    .wrapping_add((update as i16).wrapping_mul(-13)),
                                z: parent_pose.position.z.wrapping_add(1201),
                            };
                            actors.get_mut(id).unwrap().base.position = position;
                            write_vector(&mut exact, exact_id, POSITION, position);
                        }
                        write_vector(&mut exact, parent, POSITION, parent_pose.position);
                        for (field, value) in ROTATION.into_iter().zip([
                            parent_pose.rotation.pitch,
                            parent_pose.rotation.yaw,
                            parent_pose.rotation.roll,
                        ]) {
                            exact.memory.write_byte(parent + field, value.units());
                        }
                        exact.memory.write_byte(SCENE_CUE, source_cue);
                        exact.memory.write_word(CURRENT_OBJECT, parent);
                        exact.run_retail_oracle_routine(UPDATE, parent).unwrap();
                        exact.run_retail_oracle_routine(RESUME, parent).unwrap();
                        target.publish_from_parent(parent_pose);
                        let events = target.tick(cue, &actors);
                        assert_eq!(events.select_as_camera_target, update == 0);
                        assert_eq!(exact.memory.read_word(ROTATION_TARGET), object);
                        assert_eq!(exact.memory.read_word(object + PARENT_LINK), parent);
                        assert_vector(&exact, object, POSITION, target.pose.position, update);
                        assert_vector(&exact, object, OFFSET, target.attachment.offset, update);
                        assert_vector(&exact, object, VELOCITY, target.velocity, update);
                        assert_eq!(exact.memory.read_byte(object + 0x18), target.speed);
                        for (fields, rotation) in [
                            (ROTATION, target.pose.rotation),
                            (LOCAL_ROTATION, target.attachment.rotation),
                        ] {
                            for (field, expected) in fields.into_iter().zip([
                                rotation.pitch,
                                rotation.yaw,
                                rotation.roll,
                            ]) {
                                assert_eq!(
                                    exact.memory.read_byte(object + field),
                                    expected.units(),
                                    "update={update} rotation_field={field}"
                                );
                            }
                        }
                        let expected_aim = target.last_aim_actor.map_or(0, |id| {
                            actor_mapping
                                .iter()
                                .find(|(actor, _)| *actor == id)
                                .unwrap()
                                .1
                        });
                        assert_eq!(
                            exact.memory.read_word(object + AIM_LINK),
                            expected_aim,
                            "update={update}"
                        );
                        assert_eq!(
                            exact.memory.read_byte(object + 0x25) & 8 != 0,
                            events.finished
                        );
                        let path = match target.phase() {
                            OpeningTargetPhase::InitialWait { .. } => 0xFAB5,
                            OpeningTargetPhase::FirstFlight { .. } => 0xFACF,
                            OpeningTargetPhase::AwaitingOpeningEnd => 0xFAD3,
                            OpeningTargetPhase::RetargetWait { .. } => 0xFAE5,
                            OpeningTargetPhase::SecondFlight { .. } => 0xFAFE,
                            OpeningTargetPhase::AwaitingFirstCutEnd => 0xFAFF,
                            OpeningTargetPhase::Finished => 0xFB07,
                        };
                        assert_eq!(
                            exact.memory.read_word(object + FIELD_PATH),
                            path,
                            "update={update}"
                        );
                        if events.finished {
                            let before = target;
                            target.publish_from_parent(IntroScenePose::default());
                            assert_eq!(target.tick(cue, &actors), Default::default());
                            assert_eq!(target, before);
                            break;
                        }
                    }
                    assert_eq!(target.phase(), OpeningTargetPhase::Finished);
                }
            }
        }
    }
}
