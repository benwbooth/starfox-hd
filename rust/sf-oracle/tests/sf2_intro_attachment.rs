//! Run the complete retail attachment publication, including both original
//! matrix construction and point transformation, against typed scene poses.

use sf2_game::intro_motion::{IntroAttachment, IntroScenePose};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, Rotation, Vector3};

const PARENT: u16 = 0x03BD;
const CHILD: u16 = 0x03FC;
const PARENT_LINK: u16 = 6;
const POSITION: [u16; 3] = [0x0C, 0x0E, 0x10];
const ROTATION: [u16; 3] = [0x12, 0x14, 0x16];
const OFFSET: [u16; 3] = [0x1CCF, 0x1CD1, 0x1CD3];
const LOCAL_ROTATION: [u16; 3] = [0x1CD5, 0x1CD6, 0x1CD7];
const ATTACHMENT_PUBLICATION: u32 = 0x7F2229;

#[test]
fn complete_attachment_publication_matches_retail_for_combined_rotations() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rom = std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("attachment verification requires the user-owned retail SF2 ROM");
    let mut exact = Game::new(rom).unwrap();
    exact.memory.write_word(CHILD + PARENT_LINK, PARENT);
    for angle in u8::MIN..=u8::MAX {
        for offset in [
            Vector3::default(),
            Vector3 { x: 0, y: 0, z: 300 },
            Vector3 {
                x: -500,
                y: 0,
                z: 300,
            },
            Vector3 {
                x: i16::MIN,
                y: i16::MAX,
                z: -1,
            },
            Vector3 {
                x: 301,
                y: -799,
                z: 1201,
            },
        ] {
            let parent = IntroScenePose {
                position: Vector3 {
                    x: i16::MAX,
                    y: i16::MIN,
                    z: -879,
                },
                rotation: Rotation {
                    pitch: Angle::from_units(angle),
                    yaw: Angle::from_units(angle.wrapping_mul(3)),
                    roll: Angle::from_units(angle.wrapping_mul(7)),
                },
            };
            let attachment = IntroAttachment {
                offset,
                rotation: Rotation {
                    pitch: Angle::from_units(angle.wrapping_add(127)),
                    yaw: Angle::from_units(angle.wrapping_mul(13)),
                    roll: Angle::from_units(angle.wrapping_add(37)),
                },
            };
            for (object, fields, position) in
                [(PARENT, POSITION, parent.position), (CHILD, OFFSET, offset)]
            {
                for (field, value) in fields.into_iter().zip([position.x, position.y, position.z]) {
                    exact.memory.write_word(object + field, value as u16);
                }
            }
            for (object, fields, rotation) in [
                (PARENT, ROTATION, parent.rotation),
                (CHILD, LOCAL_ROTATION, attachment.rotation),
            ] {
                for (field, value) in
                    fields
                        .into_iter()
                        .zip([rotation.pitch, rotation.yaw, rotation.roll])
                {
                    exact.memory.write_byte(object + field, value.units());
                }
            }
            exact
                .run_retail_oracle_routine(ATTACHMENT_PUBLICATION, CHILD)
                .unwrap();
            let pose = attachment.world_pose(parent);
            for (field, expected) in
                POSITION
                    .into_iter()
                    .zip([pose.position.x, pose.position.y, pose.position.z])
            {
                assert_eq!(
                    exact.memory.read_word(CHILD + field) as i16,
                    expected,
                    "angle={angle} offset={offset:?} position_field={field}"
                );
            }
            for (field, expected) in ROTATION.into_iter().zip([
                pose.rotation.pitch,
                pose.rotation.yaw,
                pose.rotation.roll,
            ]) {
                assert_eq!(exact.memory.read_byte(CHILD + field), expected.units());
            }
        }
    }
}
