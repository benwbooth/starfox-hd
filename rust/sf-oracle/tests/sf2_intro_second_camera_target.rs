//! Original later-flyby camera target, created by its authored QuickSpawn.

use sf2_game::intro_motion::IntroScenePose;
use sf2_game::intro_second_camera_target::{OpeningSecondCameraTarget, OpeningSecondTargetPhase};
use sf2_game::object::{
    active_objects, allocate, ACTIVE_LIST, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY,
    PLAYER_ONE, SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, Rotation, Vector3};

const STRATEGY: u32 = 0x7F7E1E;
const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const VIEW: u16 = 0x033F;
const AUX: u16 = 0x0140;
const POSITION: [u16; 3] = [12, 14, 16];
const ROTATION: [u16; 3] = [18, 20, 22];

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("second target verification requires the user-owned retail SF2 ROM")
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

#[test]
fn complete_camera_target_matches_original_deceleration_flight_and_persistent_hold() {
    let rom = retail();
    for origin in [
        Vector3::default(),
        Vector3 {
            x: i16::MAX,
            y: i16::MIN,
            z: -701,
        },
    ] {
        for angle in [0u8, 1, 63, 127, 128, 191, 254, 255] {
            let pose = IntroScenePose {
                position: origin,
                rotation: Rotation {
                    pitch: Angle::from_units(angle),
                    yaw: Angle::from_units(angle.wrapping_mul(3)),
                    roll: Angle::from_units(angle.wrapping_mul(7)),
                },
            };
            let mut exact = Game::new(rom.clone()).unwrap();
            let parent = allocate(&mut exact.memory, 0).unwrap();
            // Begin at the real command that creates the target. Parent
            // inputs are explicit; neither its command nor its constructor
            // is replaced with a test implementation.
            exact.memory.write_word(parent + FIELD_PATH, 0xFE82);
            exact
                .memory
                .write_word(parent + FIELD_STRATEGY, STRATEGY as u16);
            exact
                .memory
                .write_byte(parent + FIELD_STRATEGY + 2, (STRATEGY >> 16) as u8);
            exact.memory.write_byte(parent + 0x2D, 1);
            exact.memory.write_word(PLAYER_ONE, VIEW);
            exact.memory.write_word(SELECTED_OBJECT, VIEW);
            exact.memory.write_word(VIEW + FIELD_PATH, AUX);
            write_pose(&mut exact, parent, pose);
            exact.memory.write_word(CURRENT_OBJECT, parent);
            exact.run_retail_oracle_routine(STRATEGY, parent).unwrap();
            let actor = active_objects(&exact.memory)
                .into_iter()
                .find(|actor| exact.memory.read_word(actor + FIELD_PATH) == 0xFB2E)
                .unwrap();
            assert_eq!(exact.memory.read_word(actor + FIELD_SHAPE), 0xBC9C);
            assert_eq!(exact.memory.read_byte(actor + 0x2D), 1);
            exact.memory.write_word(ACTIVE_LIST, actor);
            exact.memory.write_word(actor, 0);
            exact.memory.write_word(actor + 2, 0);
            let mut native = OpeningSecondCameraTarget::new(pose);
            for update in 0..400 {
                exact.memory.write_word(CURRENT_OBJECT, actor);
                exact.run_retail_oracle_routine(UPDATE, actor).unwrap();
                exact.run_retail_oracle_routine(RESUME, actor).unwrap();
                let events = native.tick();
                assert_eq!(events.select_as_camera_target, update == 0);
                assert_eq!(exact.memory.read_word(0x1DFF), actor);
                for (fields, vector) in [
                    (POSITION, native.pose.position),
                    ([50, 52, 54], native.velocity),
                ] {
                    for (field, value) in fields.into_iter().zip([vector.x, vector.y, vector.z]) {
                        assert_eq!(
                            exact.memory.read_word(actor + field) as i16,
                            value,
                            "origin={origin:?} angle={angle} update={update} field={field}"
                        );
                    }
                }
                for (field, angle) in ROTATION.into_iter().zip([
                    native.pose.rotation.pitch,
                    native.pose.rotation.yaw,
                    native.pose.rotation.roll,
                ]) {
                    assert_eq!(
                        exact.memory.read_byte(actor + field),
                        angle.units(),
                        "rotation update={update} field={field}"
                    );
                }
                assert_eq!(
                    exact.memory.read_byte(actor + 24),
                    native.speed,
                    "speed update={update}"
                );
                assert_eq!(
                    exact.memory.read_byte(actor + 11),
                    if native.is_decelerating() { 5 } else { 0 },
                    "deceleration update={update}"
                );
                assert_eq!(exact.memory.read_byte(actor + 10), 0);
                let path = match native.phase() {
                    OpeningSecondTargetPhase::Waiting { .. } => 0xFB31,
                    OpeningSecondTargetPhase::Decelerating { .. } => 0xFB3E,
                    OpeningSecondTargetPhase::ForwardFlight { .. } => 0xFB47,
                    OpeningSecondTargetPhase::Holding => 0xFB4B,
                };
                assert_eq!(
                    exact.memory.read_word(actor + FIELD_PATH),
                    path,
                    "path update={update}"
                );
                assert_eq!(exact.memory.read_byte(actor + 37) & 8, 0);
            }
            assert_eq!(native.phase(), OpeningSecondTargetPhase::Holding);
        }
    }
}
