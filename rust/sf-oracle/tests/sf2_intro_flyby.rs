//! Original actor-list scheduling, attachment publication and cleanup drive
//! the rig and all three streaks. No child path is patched or replaced.

use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_flyby::{
    OpeningFlybyEffects, OpeningFlybyRigPhase, OpeningFlybyStreakPhase, OpeningStreakDepthOrder,
    OPENING_FLYBY_STREAK_OFFSETS,
};
use sf2_game::intro_motion::{IntroAttachment, IntroScenePose};
use sf2_game::object::{
    active_objects, allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY, PLAYER_ONE,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, Rotation, Vector3};

const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const CLEANUP: u32 = 0x7F402D;
const INITIAL_STRATEGY: u32 = 0x7F7E1E;
const RIG_PATH: u16 = 0xFD5C;
const STREAK_PREPARING_PATH: u16 = 0xFDAD;
const HOLD_PATH: u16 = 0xFAB1;
const VIEW: u16 = 0x033F;
const AUX: u16 = 0x0140;
const POSITION: [u16; 3] = [0x0C, 0x0E, 0x10];
const ROTATION: [u16; 3] = [0x12, 0x14, 0x16];
const LOCAL_POSITION: [u16; 3] = [0x1CCF, 0x1CD1, 0x1CD3];
const LOCAL_ROTATION: [u16; 3] = [0x1CD5, 0x1CD6, 0x1CD7];
const OWNER: u16 = 0x1CD8;
const PARENT: u16 = 6;
const CHILD: u16 = 0x29;
const GROUP: u16 = 0x13;
const GROUP_ID: u8 = 3;
const CUE: u16 = 0x1D72;
const SHAPE_BASE: u16 = 0xBC9C;
const SHAPE_STRIDE: u16 = 28;
const REMOVAL: u16 = 0x25;
const REMOVE_BIT: u8 = 8;
const MAX_UPDATES: usize = 400;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("flyby verification requires the user-owned retail SF2 ROM")
}

fn setup_path(exact: &mut Game, actor: u16, path: u16) {
    exact.memory.write_word(actor + FIELD_PATH, path);
    exact
        .memory
        .write_word(actor + FIELD_STRATEGY, INITIAL_STRATEGY as u16);
    exact
        .memory
        .write_byte(actor + FIELD_STRATEGY + 2, (INITIAL_STRATEGY >> 16) as u8);
    exact.memory.write_word(actor + FIELD_SHAPE, SHAPE_BASE);
    exact.memory.write_byte(actor + 0x2D, 1);
}

fn write_position(exact: &mut Game, actor: u16, fields: [u16; 3], value: Vector3) {
    for (field, component) in fields.into_iter().zip([value.x, value.y, value.z]) {
        exact.memory.write_word(actor + field, component as u16);
    }
}

fn write_rotation(exact: &mut Game, actor: u16, fields: [u16; 3], value: Rotation) {
    for (field, component) in fields.into_iter().zip([value.pitch, value.yaw, value.roll]) {
        exact.memory.write_byte(actor + field, component.units());
    }
}

fn assert_position(exact: &Game, actor: u16, fields: [u16; 3], value: Vector3, update: usize) {
    for (field, component) in fields.into_iter().zip([value.x, value.y, value.z]) {
        assert_eq!(
            exact.memory.read_word(actor + field) as i16,
            component,
            "update={update}, actor={actor}, field={field}"
        );
    }
}

fn assert_rotation(exact: &Game, actor: u16, fields: [u16; 3], value: Rotation, update: usize) {
    for (field, component) in fields.into_iter().zip([value.pitch, value.yaw, value.roll]) {
        assert_eq!(
            exact.memory.read_byte(actor + field),
            component.units(),
            "update={update}, actor={actor}, field={field}"
        );
    }
}

#[test]
fn streak_depth_policy_is_the_original_sort_override_not_visibility() {
    let rom = retail();
    let expected = [
        0xB9, 0x09, 0x00, 0x89, 0x01, 0xC2, 0x20, 0xF0, 0x05, 0xA9, 0x98, 0x3A, 0x80, 0x03, 0xA9,
        0x00, 0x00, 0x9D, 0x02, 0x00,
    ];
    assert_eq!(&rom[0x01122C..0x01122C + expected.len()], &expected);
    let depth = i16::from_le_bytes([rom[0x011236], rom[0x011237]]);
    assert_eq!(
        OpeningStreakDepthOrder::Far.sort_depth_override(),
        Some(depth)
    );
    assert_eq!(
        OpeningStreakDepthOrder::Geometric.sort_depth_override(),
        None
    );
}

#[test]
fn complete_flyby_family_matches_retail_motion_schedules_and_sibling_lifetimes() {
    let rom = retail();
    for pitch in [0, 9, 10, 50, 255] {
        for opening_end in [0, 100, 182, 350] {
            for rotating in [false, true] {
                for removal_at in [None, Some(0), Some(96), Some(100), Some(150)] {
                    let local = IntroAttachment {
                        offset: Vector3 {
                            x: if rotating { i16::MAX } else { 0 },
                            y: -800,
                            z: 1_000,
                        },
                        rotation: Rotation {
                            pitch: Angle::from_units(pitch),
                            yaw: Angle::HALF_TURN,
                            roll: Angle::ZERO,
                        },
                    };
                    let mut native = OpeningFlybyEffects::new(local);
                    let mut exact = Game::new(rom.clone()).unwrap();
                    let parent = allocate(&mut exact.memory, 0).unwrap();
                    let rig = allocate(&mut exact.memory, parent).unwrap();
                    setup_path(&mut exact, parent, HOLD_PATH);
                    setup_path(&mut exact, rig, RIG_PATH);
                    exact.memory.write_word(parent + CHILD, rig);
                    exact.memory.write_byte(parent + 0x23, 0x10);
                    exact.memory.write_word(rig + PARENT, parent);
                    exact.memory.write_word(rig + OWNER, parent);
                    exact.memory.write_byte(rig + GROUP, GROUP_ID);
                    exact.memory.write_byte(rig + 0x23, 4);
                    exact.memory.write_byte(rig + 0x25, 1);
                    write_position(&mut exact, rig, LOCAL_POSITION, local.offset);
                    write_rotation(&mut exact, rig, LOCAL_ROTATION, local.rotation);
                    exact.memory.write_word(PLAYER_ONE, VIEW);
                    exact.memory.write_word(VIEW + FIELD_PATH, AUX);
                    let mut streak_ids = Vec::new();
                    let mut rig_was_removed = false;
                    for update in 0..MAX_UPDATES {
                        let forced_removal = !rig_was_removed && removal_at == Some(update);
                        if forced_removal {
                            let flags = exact.memory.read_byte(rig + REMOVAL);
                            exact.memory.write_byte(rig + REMOVAL, flags | REMOVE_BIT);
                            native.rig.request_removal();
                        }
                        let cue = if update < opening_end {
                            OpeningCameraCue::Opening
                        } else {
                            OpeningCameraCue::FirstCut
                        };
                        exact
                            .memory
                            .write_byte(CUE, if update < opening_end { 1 } else { 2 });
                        let pose = IntroScenePose {
                            position: Vector3 {
                                x: (update as i16).wrapping_mul(719),
                                y: -300,
                                z: (update as i16).wrapping_mul(337),
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
                        write_position(&mut exact, parent, POSITION, pose.position);
                        write_rotation(&mut exact, parent, ROTATION, pose.rotation);
                        exact.memory.write_word(CURRENT_OBJECT, parent);
                        exact.run_retail_oracle_routine(UPDATE, parent).unwrap();
                        exact.run_retail_oracle_routine(RESUME, parent).unwrap();
                        native.tick(pose, cue);
                        let active = active_objects(&exact.memory);
                        for &actor in &active {
                            if actor != parent && actor != rig && !streak_ids.contains(&actor) {
                                assert_eq!(
                                    exact.memory.read_word(actor + FIELD_PATH),
                                    STREAK_PREPARING_PATH,
                                    "unexpected new actor at update {update}"
                                );
                                streak_ids.push(actor);
                            }
                        }
                        streak_ids.sort_unstable();
                        assert_eq!(
                            native.streaks().count(),
                            streak_ids.len(),
                            "update {update}"
                        );
                        if !rig_was_removed {
                            assert_position(
                                &exact,
                                rig,
                                POSITION,
                                native.rig.pose.position,
                                update,
                            );
                            assert_rotation(
                                &exact,
                                rig,
                                ROTATION,
                                native.rig.pose.rotation,
                                update,
                            );
                            assert_position(
                                &exact,
                                rig,
                                LOCAL_POSITION,
                                native.rig.attachment.offset,
                                update,
                            );
                            assert_rotation(
                                &exact,
                                rig,
                                LOCAL_ROTATION,
                                native.rig.attachment.rotation,
                                update,
                            );
                            assert_eq!(
                                exact.memory.read_byte(rig + REMOVAL) & REMOVE_BIT != 0,
                                native.rig.phase() == OpeningFlybyRigPhase::Finished,
                                "update {update}"
                            );
                            if !forced_removal {
                                let path = match native.rig.phase() {
                                    OpeningFlybyRigPhase::Waiting { .. } => 0xFD5D,
                                    OpeningFlybyRigPhase::Retreating { .. } => 0xFD8E,
                                    OpeningFlybyRigPhase::ChasingSide { .. } => 0xFD94,
                                    OpeningFlybyRigPhase::AwaitingOpeningEnd => 0xFD99,
                                    OpeningFlybyRigPhase::ExitWait => 0xFDA2,
                                    OpeningFlybyRigPhase::Finished => 0xFDA2,
                                };
                                assert_eq!(
                                    exact.memory.read_word(rig + FIELD_PATH),
                                    path,
                                    "update {update}"
                                );
                            }
                        }
                        for (index, (streak, &actor)) in
                            native.streaks().zip(&streak_ids).enumerate()
                        {
                            if !active.contains(&actor) {
                                assert_eq!(streak.phase(), OpeningFlybyStreakPhase::Finished);
                                continue;
                            }
                            // Spawned by the rig, but attached to the rig's parent.
                            assert_eq!(exact.memory.read_word(actor + PARENT), parent);
                            assert_eq!(exact.memory.read_word(actor + OWNER), rig);
                            assert_eq!(exact.memory.read_byte(actor + GROUP), GROUP_ID);
                            assert_position(&exact, actor, POSITION, streak.pose.position, update);
                            assert_rotation(&exact, actor, ROTATION, streak.pose.rotation, update);
                            assert_position(
                                &exact,
                                actor,
                                LOCAL_POSITION,
                                streak.attachment.offset,
                                update,
                            );
                            assert_rotation(
                                &exact,
                                actor,
                                LOCAL_ROTATION,
                                streak.attachment.rotation,
                                update,
                            );
                            assert_eq!(
                                exact.memory.read_word(actor + FIELD_SHAPE),
                                SHAPE_BASE
                                    + streak.shape.map_or(0, |shape| shape.catalog_index() as u16
                                        * SHAPE_STRIDE)
                            );
                            assert_eq!(
                                exact.memory.read_byte(actor + 9) & 1 != 0,
                                streak.depth_order == OpeningStreakDepthOrder::Far
                            );
                            assert_eq!(
                                exact.memory.read_byte(actor + REMOVAL) & REMOVE_BIT != 0,
                                streak.phase() == OpeningFlybyStreakPhase::Finished
                            );
                            let path = match streak.phase() {
                                OpeningFlybyStreakPhase::InitialWait => unreachable!(),
                                OpeningFlybyStreakPhase::Preparing => 0xFDAD,
                                OpeningFlybyStreakPhase::Near { .. } => 0xFDB7,
                                OpeningFlybyStreakPhase::Far { .. } => 0xFDBB,
                                OpeningFlybyStreakPhase::Finished => 0xFDBD,
                            };
                            assert_eq!(
                                exact.memory.read_word(actor + FIELD_PATH),
                                path,
                                "streak {index}, update {update}"
                            );
                            if streak.phase() == OpeningFlybyStreakPhase::Preparing {
                                assert_eq!(
                                    streak.attachment.offset,
                                    OPENING_FLYBY_STREAK_OFFSETS[index]
                                );
                            }
                        }
                        exact.run_retail_oracle_routine(CLEANUP, parent).unwrap();
                        let after_cleanup = active_objects(&exact.memory);
                        rig_was_removed = !after_cleanup.contains(&rig);
                        assert_eq!(
                            rig_was_removed,
                            native.rig.phase() == OpeningFlybyRigPhase::Finished
                        );
                        for (streak, actor) in native.streaks().zip(&streak_ids) {
                            assert_eq!(
                                !after_cleanup.contains(actor),
                                streak.phase() == OpeningFlybyStreakPhase::Finished
                            );
                        }
                        if native.is_finished() {
                            assert_eq!(after_cleanup, [parent]);
                            break;
                        }
                    }
                    assert!(native.is_finished());
                    let before = native.clone();
                    native.tick(IntroScenePose::default(), OpeningCameraCue::Opening);
                    assert_eq!(native, before);
                }
            }
        }
    }
}
