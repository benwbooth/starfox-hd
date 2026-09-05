//! Original chain construction and the source follower transform.

use sf2_game::intro_motion::{follow_intro_predecessor, IntroScenePose};
use sf2_game::object::{
    active_objects, allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY, PLAYER_ONE,
    SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, Rotation, Vector3};

const STRATEGY: u32 = 0x7F7E1E;

fn retail() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("chain verification requires the user-owned retail SF2 ROM")
}

fn install_strategy(exact: &mut Game, actor: u16, path: u16) {
    exact.memory.write_word(actor + FIELD_PATH, path);
    exact
        .memory
        .write_word(actor + FIELD_STRATEGY, STRATEGY as u16);
    exact
        .memory
        .write_byte(actor + FIELD_STRATEGY + 2, (STRATEGY >> 16) as u8);
    exact.memory.write_byte(actor + 0x2D, 1);
}

#[test]
fn original_chain_constructor_uses_shared_primary_owner_and_distinct_predecessors() {
    let mut exact = Game::new(retail()).unwrap();
    let parent = allocate(&mut exact.memory, 0).unwrap();
    install_strategy(&mut exact, parent, 0xFDC2);
    exact.memory.write_word(parent + FIELD_SHAPE, 0xE194);
    exact.memory.write_byte(parent + 0x2E, 16);
    exact.memory.write_word(PLAYER_ONE, 0x033F);
    exact.memory.write_word(SELECTED_OBJECT, 0x033F);
    exact.memory.write_word(0x033F + FIELD_PATH, 0x0140);
    exact.memory.write_word(CURRENT_OBJECT, parent);
    exact.run_retail_oracle_routine(STRATEGY, parent).unwrap();
    // Retain real parent publication, without advancing its later choreography.
    exact.memory.write_word(parent + FIELD_PATH, 0xFF43);
    assert_eq!(active_objects(&exact.memory).len(), 2);
    for update in 0..16 {
        let prior_poses: Vec<_> = active_objects(&exact.memory)
            .into_iter()
            .filter(|id| *id != parent)
            .map(|id| read_pose(&exact, id))
            .collect();
        exact.memory.write_word(CURRENT_OBJECT, parent);
        exact.run_retail_oracle_routine(0x7F34E7, parent).unwrap();
        exact.run_retail_oracle_routine(0x7F354A, parent).unwrap();
        let actors = active_objects(&exact.memory);
        assert_eq!(actors.len(), 10, "same-update recursive construction");
        assert_eq!(exact.memory.read_byte(parent + 0x1CE2), 9);
        let mut predecessor = parent;
        let mut predecessor_pose = read_pose(&exact, parent);
        for (index, actor) in actors.into_iter().filter(|id| *id != parent).enumerate() {
            assert_eq!(exact.memory.read_byte(actor + 0x1CE2), index as u8 + 1);
            assert_eq!(exact.memory.read_word(actor + 6), parent, "primary owner");
            assert_eq!(
                exact.memory.read_word(actor + 0x1C),
                predecessor,
                "predecessor"
            );
            assert_eq!(
                exact.memory.read_word(actor + 0x1CD8),
                actor,
                "self-published pose"
            );
            assert_eq!(
                (exact.memory.read_word(actor + FIELD_SHAPE) - 0xBC9C) / 28,
                if index == 8 { 342 } else { 340 }
            );
            assert_eq!(exact.memory.read_word(actor + 0x1CCF), 0);
            assert_eq!(exact.memory.read_word(actor + 0x1CD1), 0);
            assert_eq!(
                exact.memory.read_word(actor + 0x1CD3) as i16,
                if index == 0 { -11 } else { -25 }
            );
            assert_eq!(
                exact.memory.read_byte(actor + 0x2D),
                if update == 0 { 2 } else { 15 }
            );
            assert_eq!(
                exact.memory.read_word(actor + FIELD_PATH),
                if update == 0 { 0xF868 } else { 0xF881 }
            );
            if update != 0 {
                predecessor_pose = follow_intro_predecessor(
                    prior_poses[index],
                    predecessor_pose,
                    if index == 0 { -11 } else { -25 },
                );
                assert_eq!(
                    predecessor_pose,
                    read_pose(&exact, actor),
                    "update={update}, ordinal={}",
                    index + 1
                );
            }
            predecessor = actor;
        }
    }
}

#[test]
fn follower_geometry_matches_original_byte_rotations_and_pre_move_facing() {
    let mut exact = Game::new(retail()).unwrap();
    let predecessor = allocate(&mut exact.memory, 0).unwrap();
    let actor = allocate(&mut exact.memory, 0).unwrap();
    exact.memory.write_word(actor + 6, predecessor);
    for depth in [-128, -127, -100, -25, -11, -1, 0, 1, 11, 25, 100, 127] {
        for pitch in 0..=255_u8 {
            for yaw in [0, 1, 31, 64, 127, 128, 192, 255] {
                let prior = pose(
                    Vector3 {
                        x: -31000,
                        y: 32760,
                        z: 17000,
                    },
                    73,
                    91,
                    pitch.wrapping_add(yaw),
                );
                let anchor = pose(
                    Vector3 {
                        x: 32740,
                        y: -32750,
                        z: -29000,
                    },
                    pitch,
                    yaw,
                    197,
                );
                compare_follow(&mut exact, actor, predecessor, prior, anchor, depth);
            }
        }
    }
    // Whole signed depth domain and wrapped/coincident aim deltas.
    for depth in i8::MIN..=i8::MAX {
        for delta in [
            Vector3::default(),
            Vector3 { x: 1, y: -1, z: 0 },
            Vector3 {
                x: -32768,
                y: 32767,
                z: -32768,
            },
            Vector3 {
                x: 6000,
                y: -13000,
                z: 28000,
            },
        ] {
            let anchor = pose(
                Vector3::default(),
                depth as u8,
                (depth as u8).wrapping_mul(17),
                63,
            );
            compare_follow(
                &mut exact,
                actor,
                predecessor,
                pose(delta, 17, 37, 251),
                anchor,
                depth,
            );
        }
    }
}

fn compare_follow(
    exact: &mut Game,
    actor: u16,
    predecessor: u16,
    prior: IntroScenePose,
    anchor: IntroScenePose,
    depth: i8,
) {
    write_pose(exact, actor, prior);
    write_pose(exact, predecessor, anchor);
    install_strategy(exact, actor, 0xF863);
    exact.memory.write_word(actor + 0x1CD3, depth as i16 as u16);
    exact.memory.write_word(CURRENT_OBJECT, actor);
    // Enter the authored command and stop at its real WaitOne; no patched code.
    exact.run_retail_oracle_routine(STRATEGY, actor).unwrap();
    assert_eq!(exact.memory.read_word(actor + FIELD_PATH), 0xF868);
    assert_eq!(
        follow_intro_predecessor(prior, anchor, depth),
        read_pose(exact, actor),
        "depth={depth}, prior={prior:?}, predecessor={anchor:?}"
    );
}

fn pose(position: Vector3, pitch: u8, yaw: u8, roll: u8) -> IntroScenePose {
    IntroScenePose {
        position,
        rotation: Rotation {
            pitch: Angle::from_units(pitch),
            yaw: Angle::from_units(yaw),
            roll: Angle::from_units(roll),
        },
    }
}

fn write_pose(exact: &mut Game, actor: u16, pose: IntroScenePose) {
    for (offset, value) in
        [12, 14, 16]
            .into_iter()
            .zip([pose.position.x, pose.position.y, pose.position.z])
    {
        exact.memory.write_word(actor + offset, value as u16);
    }
    for (offset, value) in
        [18, 20, 22]
            .into_iter()
            .zip([pose.rotation.pitch, pose.rotation.yaw, pose.rotation.roll])
    {
        exact.memory.write_byte(actor + offset, value.units());
    }
}

fn read_pose(exact: &Game, actor: u16) -> IntroScenePose {
    pose(
        Vector3 {
            x: exact.memory.read_word(actor + 12) as i16,
            y: exact.memory.read_word(actor + 14) as i16,
            z: exact.memory.read_word(actor + 16) as i16,
        },
        exact.memory.read_byte(actor + 18),
        exact.memory.read_byte(actor + 20),
        exact.memory.read_byte(actor + 22),
    )
}
