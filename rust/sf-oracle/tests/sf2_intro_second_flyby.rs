//! Source-authored later-flyby placement tables and attached trail path.

use sf2_game::intro_motion::IntroScenePose;
use sf2_game::intro_second_flyby::{
    OpeningSecondFlybyFlare, OpeningSecondFlybyPlacement, OpeningSecondFlybyTrail,
};
use sf2_game::object::{
    active_objects, allocate, ACTIVE_LIST, CURRENT_OBJECT, FIELD_PATH, FIELD_STRATEGY, PLAYER_ONE,
    SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, Behavior, Object, ObjectKind, ObjectStore, Rotation, ShapeId, Vector3};

const STRATEGY: u32 = 0x7F7E1E;
const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const POSITION: [u16; 3] = [12, 14, 16];
const ROTATION: [u16; 3] = [18, 20, 22];

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("later flyby verification requires the user-owned retail SF2 ROM")
}

#[test]
fn source_spawned_engine_flare_retains_common_attachment_updates_without_a_timeout() {
    let rom = retail();
    for rotating in [false, true] {
        for translated in [false, true] {
            let mut exact = Game::new(rom.clone()).unwrap();
            let parent = allocate(&mut exact.memory, 0).unwrap();
            exact.memory.write_word(parent + FIELD_PATH, 0xFE11);
            exact
                .memory
                .write_word(parent + FIELD_STRATEGY, STRATEGY as u16);
            exact
                .memory
                .write_byte(parent + FIELD_STRATEGY + 2, (STRATEGY >> 16) as u8);
            exact.memory.write_byte(parent + 0x2D, 1);
            exact.memory.write_word(PLAYER_ONE, 0x033F);
            exact.memory.write_word(SELECTED_OBJECT, 0x033F);
            exact.memory.write_word(0x033F + FIELD_PATH, 0x0140);
            exact.memory.write_word(CURRENT_OBJECT, parent);
            // Execute the original parent spawn and its remaining path body.
            // The newly created flare has not executed its own strategy yet.
            exact.run_retail_oracle_routine(STRATEGY, parent).unwrap();
            let actor = exact.memory.read_word(parent);
            assert_eq!(exact.memory.read_word(actor + FIELD_PATH), 0xFD58);
            assert_eq!(active_objects(&exact.memory), vec![parent, actor]);
            exact.memory.write_word(parent + FIELD_PATH, 0xFF43);
            exact.memory.write_byte(parent + 24, 0);
            for field in [50, 52, 54] {
                exact.memory.write_word(parent + field, 0);
            }
            let mut objects = ObjectStore::new();
            let parent_id = objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Effect,
                ))
                .unwrap();
            let mut native = OpeningSecondFlybyFlare::new(parent_id);
            // This independent parent completes an attachment pass after its
            // path body, so the new flare is published before its first tick.
            native.publish_from_parent(parent_id, read_flare_pose(&exact, parent));
            assert_eq!(native.pose, read_flare_pose(&exact, actor));
            assert_eq!(exact.memory.read_word(actor + 6), parent);
            assert_eq!(exact.memory.read_word(actor + 0x1CD8), parent);
            assert_eq!(
                exact.memory.read_byte(actor + 0x21) & 1 != 0,
                native.contact_disabled()
            );
            assert_eq!(
                exact.memory.read_byte(actor + 9) & 1 != 0,
                native.sort_override()
            );
            for (field, value) in [0x1CCF, 0x1CD1, 0x1CD3].into_iter().zip([
                native.attachment().offset.x,
                native.attachment().offset.y,
                native.attachment().offset.z,
            ]) {
                assert_eq!(exact.memory.read_word(actor + field) as i16, value);
            }
            for (field, value) in [0x1CD5, 0x1CD6, 0x1CD7].into_iter().zip([
                native.attachment().rotation.pitch,
                native.attachment().rotation.yaw,
                native.attachment().rotation.roll,
            ]) {
                assert_eq!(exact.memory.read_byte(actor + field), value.units());
            }
            for update in 0..400_i16 {
                let parent_pose = IntroScenePose {
                    position: if translated {
                        Vector3 {
                            x: i16::MAX.wrapping_add(update * 37),
                            y: i16::MIN.wrapping_sub(update * 19),
                            z: update.wrapping_mul(239).wrapping_sub(29000),
                        }
                    } else {
                        Vector3::default()
                    },
                    rotation: if rotating {
                        Rotation {
                            pitch: Angle::from_units((update as u8).wrapping_mul(13)),
                            yaw: Angle::from_units((update as u8).wrapping_mul(31)),
                            roll: Angle::from_units((update as u8).wrapping_mul(47)),
                        }
                    } else {
                        Rotation::default()
                    },
                };
                write_flare_pose(&mut exact, parent, parent_pose);
                exact.memory.write_word(CURRENT_OBJECT, parent);
                exact.run_retail_oracle_routine(UPDATE, parent).unwrap();
                exact.run_retail_oracle_routine(RESUME, parent).unwrap();
                native.publish_from_parent(parent_id, parent_pose);
                native.tick();
                assert_eq!(
                    native.pose,
                    read_flare_pose(&exact, actor),
                    "update={update} rotating={rotating} translated={translated}"
                );
                assert_eq!(
                    exact.memory.read_byte(actor + 0x21) & 1 != 0,
                    native.contact_disabled()
                );
                assert_eq!(
                    exact.memory.read_byte(actor + 9) & 1 != 0,
                    native.sort_override()
                );
                assert_eq!(exact.memory.read_byte(actor + 9) & 8, 8);
                assert_eq!(exact.memory.read_word(actor + FIELD_STRATEGY), 0x9DDE);
                assert_eq!(exact.memory.read_byte(actor + FIELD_STRATEGY + 2), 0x7F);
                assert_eq!(exact.memory.read_word(actor + FIELD_PATH), 0xFD5B);
                assert_eq!(
                    exact.memory.read_word(actor + 4),
                    0xBC9C + native.shape().catalog_index() as u16 * 28
                );
                exact.run_retail_oracle_routine(0x7F402D, parent).unwrap();
                assert_eq!(active_objects(&exact.memory), vec![parent, actor]);
            }
        }
    }
}

fn write_flare_pose(exact: &mut Game, actor: u16, pose: IntroScenePose) {
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

fn read_flare_pose(exact: &Game, actor: u16) -> IntroScenePose {
    IntroScenePose {
        position: Vector3 {
            x: exact.memory.read_word(actor + 12) as i16,
            y: exact.memory.read_word(actor + 14) as i16,
            z: exact.memory.read_word(actor + 16) as i16,
        },
        rotation: Rotation {
            pitch: Angle::from_units(exact.memory.read_byte(actor + 18)),
            yaw: Angle::from_units(exact.memory.read_byte(actor + 20)),
            roll: Angle::from_units(exact.memory.read_byte(actor + 22)),
        },
    }
}

#[test]
fn indexed_poses_match_all_authored_position_and_rotation_channels() {
    let rom = retail();
    for (index, placement) in [
        (16, OpeningSecondFlybyPlacement::Arrival),
        (17, OpeningSecondFlybyPlacement::MiddleCut),
        (18, OpeningSecondFlybyPlacement::FinalCut),
        (19, OpeningSecondFlybyPlacement::DepartingCraft),
    ] {
        let pose = placement.pose();
        for (table, value) in [
            (0x3FECC, pose.position.x),
            (0x3FF0A, pose.position.y),
            (0x3FF48, pose.position.z),
        ] {
            assert_eq!(
                i16::from_le_bytes(
                    rom[table + index * 2..table + index * 2 + 2]
                        .try_into()
                        .unwrap()
                ),
                value
            );
        }
        for (table, angle) in [
            (0x3FF86, pose.rotation.pitch),
            (0x3FFA5, pose.rotation.yaw),
            (0x3FFC4, pose.rotation.roll),
        ] {
            assert_eq!(rom[table + index], angle.units());
        }
    }
}

#[test]
fn source_spawned_trail_matches_parent_publication_local_motion_and_end() {
    let rom = retail();
    for rotating in [false, true] {
        for translated in [false, true] {
            let mut exact = Game::new(rom.clone()).unwrap();
            let parent = allocate(&mut exact.memory, 0).unwrap();
            exact.memory.write_word(parent + FIELD_PATH, 0xFE58);
            exact
                .memory
                .write_word(parent + FIELD_STRATEGY, STRATEGY as u16);
            exact
                .memory
                .write_byte(parent + FIELD_STRATEGY + 2, (STRATEGY >> 16) as u8);
            exact.memory.write_byte(parent + 0x2D, 1);
            exact.memory.write_word(PLAYER_ONE, 0x033F);
            exact.memory.write_word(SELECTED_OBJECT, 0x033F);
            exact.memory.write_word(0x033F + FIELD_PATH, 0x0140);
            exact.memory.write_byte(0x1D72, 1);
            exact.memory.write_word(CURRENT_OBJECT, parent);
            exact.run_retail_oracle_routine(STRATEGY, parent).unwrap();
            let actor = active_objects(&exact.memory)
                .into_iter()
                .find(|actor| exact.memory.read_word(actor + FIELD_PATH) == 0xFF8D)
                .unwrap();
            let mut native = OpeningSecondFlybyTrail::new();
            for (field, value) in [0x1CCF, 0x1CD1, 0x1CD3].into_iter().zip([
                native.attachment.offset.x,
                native.attachment.offset.y,
                native.attachment.offset.z,
            ]) {
                assert_eq!(exact.memory.read_word(actor + field) as i16, value);
            }
            assert_eq!(
                exact.memory.read_word(actor + 4),
                0xBC9C + native.shape().catalog_index() as u16 * 28
            );
            assert_eq!(exact.memory.read_word(ACTIVE_LIST), parent);
            for update in 0..20 {
                let mut parent_pose = IntroScenePose {
                    position: if translated {
                        Vector3 {
                            x: i16::MAX.wrapping_add(update * 19),
                            y: i16::MIN.wrapping_sub(update * 13),
                            z: -701,
                        }
                    } else {
                        Vector3::default()
                    },
                    rotation: if rotating {
                        Rotation {
                            pitch: Angle::from_units(update as u8 * 3),
                            yaw: Angle::from_units((update as u8).wrapping_mul(17)),
                            roll: Angle::from_units((update as u8).wrapping_mul(23)),
                        }
                    } else {
                        Rotation::default()
                    },
                };
                for (field, value) in POSITION.into_iter().zip([
                    parent_pose.position.x,
                    parent_pose.position.y,
                    parent_pose.position.z,
                ]) {
                    exact.memory.write_word(parent + field, value as u16);
                }
                for (field, angle) in ROTATION.into_iter().zip([
                    parent_pose.rotation.pitch,
                    parent_pose.rotation.yaw,
                    parent_pose.rotation.roll,
                ]) {
                    exact.memory.write_byte(parent + field, angle.units());
                }
                // The unchanged parent wait loop increments yaw once, then
                // publishes its attached child before that child's strategy.
                parent_pose.rotation.yaw = parent_pose.rotation.yaw.wrapping_add(1);
                exact.memory.write_word(CURRENT_OBJECT, parent);
                exact.run_retail_oracle_routine(UPDATE, parent).unwrap();
                exact.run_retail_oracle_routine(RESUME, parent).unwrap();
                native.publish_from_parent(parent_pose);
                let ended = native.tick();
                for (fields, vector) in [
                    (POSITION, native.pose.position),
                    ([0x1CCF, 0x1CD1, 0x1CD3], native.attachment.offset),
                ] {
                    for (field, value) in fields.into_iter().zip([vector.x, vector.y, vector.z]) {
                        assert_eq!(exact.memory.read_word(actor + field) as i16, value, "update={update} field={field} rotating={rotating} translated={translated}");
                    }
                }
                for (field, angle) in ROTATION.into_iter().zip([
                    native.pose.rotation.pitch,
                    native.pose.rotation.yaw,
                    native.pose.rotation.roll,
                ]) {
                    assert_eq!(exact.memory.read_byte(actor + field), angle.units());
                }
                assert_eq!(
                    exact.memory.read_byte(actor + 0x1CEF),
                    native.depth_offset()
                );
                assert_eq!(exact.memory.read_byte(actor + 0x25) & 8 != 0, ended);
                assert_eq!(
                    exact.memory.read_word(actor + FIELD_PATH),
                    if ended { 0xFF97 } else { 0xFF93 }
                );
            }
            assert!(native.is_finished());
        }
    }
}
