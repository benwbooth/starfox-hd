//! Execute the complete camera path, including its scheduled view update,
//! directly from the retail ROM. Machine state is confined to this verifier.

use sf2_game::intro_camera::{
    IntroCameraView, OpeningCameraCue, OpeningCameraPhase, OpeningCameraRig,
    OPENING_CAMERA_WAYPOINTS,
};
use sf2_game::intro_logo::LogoSceneScroll;
use sf2_game::intro_motion::AttractCameraAngles;
use sf2_game::object::{allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_STRATEGY, PLAYER_ONE};
use sf2_game::oracle_compat::Game;
use sf2_game::Vector3;

const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const INITIAL_STRATEGY: u32 = 0x7F7E1E;
const CAMERA_PATH: u16 = 0xFB4C;
const VIEW: u16 = 0x033F;
const TARGET: u16 = 0x03FC;
const SELECTED_AUX: u16 = 0x0140;
const POSITION: [u16; 3] = [0x0C, 0x0E, 0x10];
const VELOCITY: [u16; 3] = [0x32, 0x34, 0x36];
const ROTATION: [u16; 3] = [0x12, 0x14, 0x16];
const SCENE_CUE: u16 = 0x1D72;
const ROTATION_TARGET: u16 = 0x1DFF;
const HORIZONTAL_SCROLL: u16 = 0x1E1C;
const DEPTH_SCROLL: u16 = 0x1E20;
const HORIZONTAL_POLICY: u16 = 0x6B77 + SELECTED_AUX;
const WAYPOINT_INDEX: u16 = 0x2E;
const MAX_UPDATES: usize = 180;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("camera differential tests require the user-owned retail SF2 ROM")
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

#[test]
fn camera_waypoints_are_authored_rom_coordinates() {
    let rom = retail();
    for (index, point) in OPENING_CAMERA_WAYPOINTS.into_iter().enumerate() {
        for (base, expected) in [(0x3FE27, point.x), (0x3FE35, point.y), (0x3FE43, point.z)] {
            let offset = base + index * 2;
            assert_eq!(i16::from_le_bytes([rom[offset], rom[offset + 1]]), expected);
        }
    }
}

#[test]
fn complete_opening_camera_path_matches_retail_cuts_waits_and_view_publication() {
    let rom = retail();
    for cut_spacing in [1, 7, 20] {
        for final_cut_early in [false, true] {
            for scrolling in [false, true] {
                let origin = Vector3 {
                    x: i16::MAX,
                    y: -177,
                    z: i16::MIN,
                };
                let mut rig = OpeningCameraRig::new(origin);
                let mut view = IntroCameraView {
                    position: Vector3 {
                        x: 901,
                        y: 171,
                        z: -879,
                    },
                    angles: AttractCameraAngles {
                        pitch: 197,
                        yaw: 301,
                        roll: 32_771,
                    },
                };
                let mut exact = Game::new(rom.clone()).unwrap();
                let object = allocate(&mut exact.memory, 0).unwrap();
                exact.memory.write_word(object + FIELD_PATH, CAMERA_PATH);
                exact
                    .memory
                    .write_word(object + FIELD_STRATEGY, INITIAL_STRATEGY as u16);
                exact
                    .memory
                    .write_byte(object + FIELD_STRATEGY + 2, (INITIAL_STRATEGY >> 16) as u8);
                exact.memory.write_byte(object + 0x2D, 1);
                exact.memory.write_word(PLAYER_ONE, VIEW);
                exact.memory.write_word(VIEW + FIELD_PATH, SELECTED_AUX);
                exact.memory.write_word(ROTATION_TARGET, TARGET);
                write_vector(&mut exact, object, POSITION, origin);
                write_vector(&mut exact, VIEW, POSITION, view.position);
                for (field, value) in
                    ROTATION
                        .into_iter()
                        .zip([view.angles.pitch, view.angles.yaw, view.angles.roll])
                {
                    exact.memory.write_word(VIEW + field, value);
                }
                for update in 0..MAX_UPDATES {
                    let cut = update.saturating_sub(2) / cut_spacing;
                    let (cue, source_cue) = match cut {
                        0 => (OpeningCameraCue::Opening, 1),
                        1 => (OpeningCameraCue::FirstCut, 2),
                        2 => (OpeningCameraCue::SecondCut, 3),
                        3 => (OpeningCameraCue::ThirdCut, 4),
                        _ if !final_cut_early && update < MAX_UPDATES - 20 => {
                            (OpeningCameraCue::FourthCut, 5)
                        }
                        4 => (OpeningCameraCue::FourthCut, 5),
                        _ => (OpeningCameraCue::FinalCut, 6),
                    };
                    let scroll = if scrolling {
                        LogoSceneScroll {
                            horizontal: -19,
                            depth: (update as i16 % 17) - 8,
                            horizontal_locked: update % 2 == 0,
                        }
                    } else {
                        LogoSceneScroll::default()
                    };
                    let target = Vector3 {
                        x: (update as i16).wrapping_mul(701),
                        y: (update as i16).wrapping_mul(-409),
                        z: (update as i16).wrapping_mul(149),
                    };
                    exact.memory.write_byte(SCENE_CUE, source_cue);
                    exact
                        .memory
                        .write_word(HORIZONTAL_SCROLL, scroll.horizontal as u16);
                    exact.memory.write_word(DEPTH_SCROLL, scroll.depth as u16);
                    exact.memory.write_byte(
                        HORIZONTAL_POLICY,
                        if scroll.horizontal_locked { 4 } else { 0 },
                    );
                    write_vector(&mut exact, TARGET, POSITION, target);
                    exact.memory.write_word(CURRENT_OBJECT, object);
                    exact.run_retail_oracle_routine(UPDATE, object).unwrap();
                    exact.run_retail_oracle_routine(RESUME, object).unwrap();
                    rig.tick(cue, scroll.depth, target, &mut view);
                    assert_vector(&exact, object, POSITION, rig.position, update);
                    assert_vector(&exact, object, VELOCITY, rig.velocity, update);
                    assert_vector(&exact, VIEW, POSITION, view.position, update);
                    for (field, expected) in ROTATION.into_iter().zip([
                        view.angles.pitch,
                        view.angles.yaw,
                        view.angles.roll,
                    ]) {
                        assert_eq!(
                            exact.memory.read_word(VIEW + field),
                            expected,
                            "update={update} rotation_field={field}"
                        );
                    }
                    assert_eq!(
                        usize::from(exact.memory.read_byte(object + WAYPOINT_INDEX)),
                        rig.cuts_taken()
                    );
                    let path = match rig.phase() {
                        OpeningCameraPhase::InitialWait => unreachable!(),
                        OpeningCameraPhase::FollowingScroll if update == 0 => 0xFB4E,
                        OpeningCameraPhase::FollowingScroll => 0xFB51,
                        OpeningCameraPhase::AwaitingSecondCut => 0xFB60,
                        OpeningCameraPhase::AwaitingThirdCut => 0xFB6D,
                        OpeningCameraPhase::AwaitingFourthCut => 0xFB78,
                        OpeningCameraPhase::SlowFlight { .. } => 0xFB85,
                        OpeningCameraPhase::FlightPause { .. } => 0xFB8A,
                        OpeningCameraPhase::AimedFlight { .. } => 0xFB8E,
                        OpeningCameraPhase::AwaitingFinalCut => 0xFB9B,
                        OpeningCameraPhase::Holding => 0xFBB0,
                    };
                    assert_eq!(
                        exact.memory.read_word(object + FIELD_PATH),
                        path,
                        "update={update}"
                    );
                }
                assert_eq!(rig.phase(), OpeningCameraPhase::Holding);
                assert_eq!(rig.cuts_taken(), OPENING_CAMERA_WAYPOINTS.len());
            }
        }
    }
}
