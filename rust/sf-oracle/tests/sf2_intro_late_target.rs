//! Run the source-created later target, its unmodified child strategy, the
//! original actor-list dispatcher, attachment service and cleanup together.

use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_late_target::{OpeningLateCameraTarget, OpeningLateTargetPhase};
use sf2_game::intro_motion::IntroScenePose;
use sf2_game::object::{
    active_objects, allocate, ACTIVE_LIST, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY,
    PLAYER_ONE, SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, Rotation, Vector3};

const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const CLEANUP: u32 = 0x7F402D;
const STRATEGY: u32 = 0x7F7E1E;
const TARGET_PATH: u16 = 0xFB08;
const VIEW: u16 = 0x033F;
const AUX: u16 = 0x0140;
const POSITION: [u16; 3] = [0x0C, 0x0E, 0x10];
const ROTATION: [u16; 3] = [0x12, 0x14, 0x16];
const VELOCITY: [u16; 3] = [0x32, 0x34, 0x36];
const OFFSET: [u16; 3] = [0x1CCF, 0x1CD1, 0x1CD3];
const LOCAL_ROTATION: [u16; 3] = [0x1CD5, 0x1CD6, 0x1CD7];
const CUE: u16 = 0x1D72;
const ROTATION_TARGET: u16 = 0x1DFF;
const SHAPE_BASE: u16 = 0xBC9C;
const SHAPE_STRIDE: u16 = 28;
const FLAGS: u16 = 0x25;
const REMOVE_BIT: u8 = 8;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("late target verification requires the user-owned retail SF2 ROM")
}

fn authored_target(rom: &[u8]) -> (Game, u16) {
    let mut exact = Game::new(rom.to_vec()).unwrap();
    let root = allocate(&mut exact.memory, 0).unwrap();
    exact.memory.write_word(root + FIELD_PATH, 0xFA11);
    exact
        .memory
        .write_word(root + FIELD_STRATEGY, STRATEGY as u16);
    exact
        .memory
        .write_byte(root + FIELD_STRATEGY + 2, (STRATEGY >> 16) as u8);
    exact.memory.write_byte(root + 0x2D, 1);
    exact.memory.write_word(root + FIELD_SHAPE, SHAPE_BASE);
    exact.memory.write_word(PLAYER_ONE, VIEW);
    exact.memory.write_word(SELECTED_OBJECT, VIEW);
    exact.memory.write_word(VIEW + FIELD_PATH, AUX);
    for update in 0..=293 {
        let cue = match update {
            0..182 => 1,
            182..249 => 2,
            249..293 => 3,
            _ => 4,
        };
        exact.memory.write_byte(CUE, cue);
        exact.memory.write_word(CURRENT_OBJECT, root);
        let strategy = u32::from(exact.memory.read_word(root + FIELD_STRATEGY))
            | (u32::from(exact.memory.read_byte(root + FIELD_STRATEGY + 2)) << 16);
        exact.run_retail_oracle_routine(strategy, root).unwrap();
    }
    let target = active_objects(&exact.memory)
        .into_iter()
        .find(|actor| exact.memory.read_word(actor + FIELD_PATH) == TARGET_PATH)
        .unwrap();
    assert_eq!(exact.memory.read_byte(target + 0x2D), 1);
    assert_eq!(exact.memory.read_byte(target + 0x2E), 15);
    assert_eq!(exact.memory.read_word(target + FIELD_SHAPE), SHAPE_BASE);
    assert_eq!(exact.memory.read_word(target + 6), 0);
    exact.memory.write_word(ACTIVE_LIST, target);
    exact.memory.write_word(target, 0);
    exact.memory.write_word(target + 2, 0);
    (exact, target)
}

fn assert_vector(exact: &Game, actor: u16, fields: [u16; 3], value: Vector3, update: usize) {
    for (field, component) in fields.into_iter().zip([value.x, value.y, value.z]) {
        assert_eq!(
            exact.memory.read_word(actor + field) as i16,
            component,
            "update {update}, actor {actor}, field {field}"
        );
    }
}

fn assert_rotation(exact: &Game, actor: u16, fields: [u16; 3], value: Rotation, update: usize) {
    for (field, component) in fields.into_iter().zip([value.pitch, value.yaw, value.roll]) {
        assert_eq!(
            exact.memory.read_byte(actor + field),
            component.units(),
            "update {update}, actor {actor}, field {field}"
        );
    }
}

#[test]
fn later_target_family_matches_original_camera_cuts_motion_and_parent_cleanup() {
    let rom = retail();
    for fourth_cut in [
        None,
        Some(0),
        Some(14),
        Some(15),
        Some(16),
        Some(17),
        Some(33),
        Some(34),
        Some(46),
        Some(47),
        Some(48),
    ] {
        for one_update_cut in [false, true] {
            for removal_at in [
                None,
                Some(0),
                Some(14),
                Some(15),
                Some(16),
                Some(17),
                Some(33),
                Some(46),
            ] {
                let (mut exact, target) = authored_target(&rom);
                let inherited = IntroScenePose {
                    position: Vector3 {
                        x: i16::MAX,
                        y: i16::MIN,
                        z: 791,
                    },
                    rotation: Rotation {
                        pitch: Angle::from_units(79),
                        yaw: Angle::from_units(93),
                        roll: Angle::from_units(157),
                    },
                };
                for (field, value) in POSITION.into_iter().zip([
                    inherited.position.x,
                    inherited.position.y,
                    inherited.position.z,
                ]) {
                    exact.memory.write_word(target + field, value as u16);
                }
                for (field, value) in ROTATION.into_iter().zip([
                    inherited.rotation.pitch,
                    inherited.rotation.yaw,
                    inherited.rotation.roll,
                ]) {
                    exact.memory.write_byte(target + field, value.units());
                }
                let mut native = OpeningLateCameraTarget::new(inherited);
                let mut child = None;
                for update in 0..50 {
                    if removal_at == Some(update) {
                        let flags = exact.memory.read_byte(target + FLAGS);
                        exact.memory.write_byte(target + FLAGS, flags | REMOVE_BIT);
                        native.request_removal();
                    }
                    let at_cut = fourth_cut.is_some_and(|cut| {
                        if one_update_cut {
                            update == cut
                        } else {
                            update >= cut
                        }
                    });
                    let cue = if at_cut {
                        OpeningCameraCue::FourthCut
                    } else {
                        OpeningCameraCue::ThirdCut
                    };
                    exact.memory.write_byte(CUE, if at_cut { 5 } else { 4 });
                    // Neither actor opts into ambient scene scroll.
                    exact
                        .memory
                        .write_word(0x1E1C, (update as u16).wrapping_mul(719));
                    exact
                        .memory
                        .write_word(0x1E20, (update as u16).wrapping_mul(337));
                    exact.memory.write_word(CURRENT_OBJECT, target);
                    exact.run_retail_oracle_routine(UPDATE, target).unwrap();
                    exact.run_retail_oracle_routine(RESUME, target).unwrap();
                    let events = native.tick(cue);
                    assert_eq!(events.select_as_camera_target, update == 0);
                    assert_eq!(events.spawn_effect, update == 0);
                    assert_eq!(exact.memory.read_word(ROTATION_TARGET), target);
                    assert_vector(&exact, target, POSITION, native.pose.position, update);
                    assert_rotation(&exact, target, ROTATION, native.pose.rotation, update);
                    assert_vector(&exact, target, VELOCITY, native.velocity, update);
                    assert_eq!(exact.memory.read_byte(target + 0x18), native.speed);
                    let active = active_objects(&exact.memory);
                    if update == 0 {
                        child = active.iter().copied().find(|actor| *actor != target);
                        assert_eq!(active.len(), 2);
                    }
                    let child = child.unwrap();
                    let effect = native.effect.as_ref().unwrap();
                    if active.contains(&child) {
                        assert_vector(&exact, child, POSITION, effect.pose.position, update);
                        assert_rotation(&exact, child, ROTATION, effect.pose.rotation, update);
                        assert_vector(&exact, child, OFFSET, effect.attachment.offset, update);
                        assert_rotation(
                            &exact,
                            child,
                            LOCAL_ROTATION,
                            effect.attachment.rotation,
                            update,
                        );
                        assert_vector(&exact, child, VELOCITY, effect.velocity, update);
                        assert_eq!(exact.memory.read_byte(child + 0x18), effect.speed);
                        assert_eq!(
                            exact.memory.read_word(child + FIELD_SHAPE),
                            SHAPE_BASE + effect.shape().catalog_index() as u16 * SHAPE_STRIDE
                        );
                        assert_eq!(exact.memory.read_word(child + 6), target);
                        assert_eq!(exact.memory.read_word(child + 0x1CD8), target);
                        assert_eq!(exact.memory.read_byte(child + 0x13), 1);
                    }
                    exact.run_retail_oracle_routine(CLEANUP, target).unwrap();
                    let active = active_objects(&exact.memory);
                    assert_eq!(
                        !active.contains(&target),
                        native.is_finished(),
                        "update {update}, cut {fourth_cut:?}, removal {removal_at:?}"
                    );
                    assert_eq!(
                        !active.contains(&child),
                        !effect.is_visible(),
                        "update {update}, cut {fourth_cut:?}, removal {removal_at:?}"
                    );
                    // The source retains the last selected target identity even
                    // after actor cleanup; a scene consumer must not fabricate
                    // a target-clear event on removal.
                    assert_eq!(exact.memory.read_word(ROTATION_TARGET), target);
                    if native.is_finished() {
                        assert!(active.is_empty());
                        let before = native;
                        assert_eq!(native.tick(cue), Default::default());
                        assert_eq!(native, before);
                        break;
                    }
                }
                assert_eq!(native.phase(), OpeningLateTargetPhase::Finished);
            }
        }
    }
}
