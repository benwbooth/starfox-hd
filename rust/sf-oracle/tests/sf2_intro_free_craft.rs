//! Execute the original free-craft strategy and its actual selected-player
//! effect service. No auxiliary call or path instruction is patched out.

use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_free_craft::{IntroAuxiliaryEffect, OpeningFreeCraft, OpeningFreeCraftPhase};
use sf2_game::intro_motion::IntroScenePose;
use sf2_game::object::{
    active_objects, allocate, ACTIVE_LIST, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY,
    PLAYER_ONE, SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{
    Angle, Behavior, Object, ObjectId, ObjectKind, ObjectStore, Rotation, ShapeId, Vector3,
};

const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const STRATEGY: u32 = 0x7F7E1E;
const PATH: u16 = 0xFCC5;
const AUX_SERVICE: u32 = 0x07B6EF;
const VIEW: u16 = 0x033F;
const AUX: u16 = 0x0140;
const POSITION: [u16; 3] = [0x0C, 0x0E, 0x10];
const VELOCITY: [u16; 3] = [0x32, 0x34, 0x36];
const ROTATION: [u16; 3] = [0x12, 0x14, 0x16];
const EFFECT_ORIGIN: [u16; 3] = [0x6A92, 0x6A94, 0x6A96];
const EFFECT_OWNER: u16 = 0x6A98;
const EFFECT_FLAGS: u16 = 0x6A8C;
const EFFECT_AXES: [u16; 3] = [0x6A8D, 0x6A8E, 0x6A8F];
const EFFECT_CONTROLS: [u16; 3] = [0x6C2A, 0x6C2B, 0x6C2C];
const CUE: u16 = 0x1D72;
const MARKER: u16 = 0x1C31;
const MARKER_CLASS: u16 = 0x1C35;
const SHAPE_BASE: u16 = 0xBC9C;
const SHAPE_STRIDE: u16 = 28;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("free-craft verification requires the user-owned retail SF2 ROM")
}

fn native_id() -> ObjectId {
    ObjectStore::new()
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::from_catalog_index(64),
            Behavior::Effect,
        ))
        .unwrap()
}

fn authored_craft(rom: &[u8]) -> (Game, u16) {
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
    for _ in 0..=96 {
        exact.memory.write_word(CURRENT_OBJECT, root);
        let strategy = u32::from(exact.memory.read_word(root + FIELD_STRATEGY))
            | (u32::from(exact.memory.read_byte(root + FIELD_STRATEGY + 2)) << 16);
        exact.run_retail_oracle_routine(strategy, root).unwrap();
    }
    let craft = active_objects(&exact.memory)
        .into_iter()
        .find(|actor| exact.memory.read_word(actor + FIELD_PATH) == PATH)
        .unwrap();
    assert_eq!(exact.memory.read_byte(craft + 0x2D), 1);
    assert_eq!(exact.memory.read_byte(craft + 0x2E), 5);
    assert_eq!(
        exact.memory.read_word(craft + FIELD_SHAPE),
        SHAPE_BASE + 64 * SHAPE_STRIDE
    );
    // Isolate the freshly source-constructed craft, not an approximation of
    // its constructor flags. Other opening actors are verified separately.
    exact.memory.write_word(ACTIVE_LIST, craft);
    exact.memory.write_word(craft, 0);
    exact.memory.write_word(craft + 2, 0);
    (exact, craft)
}

fn write_vector(exact: &mut Game, base: u16, fields: [u16; 3], vector: Vector3) {
    for (field, value) in fields.into_iter().zip([vector.x, vector.y, vector.z]) {
        exact.memory.write_word(base + field, value as u16);
    }
}

fn assert_vector(exact: &Game, base: u16, fields: [u16; 3], vector: Vector3, update: usize) {
    for (field, value) in fields.into_iter().zip([vector.x, vector.y, vector.z]) {
        assert_eq!(
            exact.memory.read_word(base + field) as i16,
            value,
            "update {update}, field {field}"
        );
    }
}

fn seed_effect(
    exact: &mut Game,
    actor: u16,
    id: ObjectId,
    flags: u8,
    owned: bool,
) -> IntroAuxiliaryEffect {
    let effect = IntroAuxiliaryEffect {
        frozen: flags & 128 != 0,
        tracking: flags & 64 != 0,
        axis_modes: [11, 13, 17],
        range: -901,
        origin: Vector3 {
            x: 311,
            y: 919,
            z: -157,
        },
        owner: owned.then_some(id),
        transition_mode: 23,
        limit: 701,
        remaining: -401,
        target_axis: 19,
        target_control: 29,
        axis_controls: [7, 9, 13],
    };
    exact.memory.write_word(PLAYER_ONE, VIEW);
    exact.memory.write_word(VIEW + FIELD_PATH, AUX);
    exact.memory.write_byte(AUX + EFFECT_FLAGS, flags);
    write_vector(exact, AUX, EFFECT_ORIGIN, effect.origin);
    exact
        .memory
        .write_word(AUX + EFFECT_OWNER, if owned { actor } else { VIEW });
    for (fields, values) in [
        (EFFECT_AXES, effect.axis_modes),
        (EFFECT_CONTROLS, effect.axis_controls),
    ] {
        for (field, value) in fields.into_iter().zip(values) {
            exact.memory.write_byte(AUX + field, value);
        }
    }
    for (field, value) in [
        (0x6A90, effect.range as u16),
        (0x6C1C, effect.transition_mode),
        (0x6C24, effect.limit),
        (0x6C26, effect.remaining as u16),
    ] {
        exact.memory.write_word(AUX + field, value);
    }
    exact.memory.write_byte(AUX + 0x6C29, effect.target_axis);
    exact.memory.write_byte(AUX + 0x6C28, effect.target_control);
    effect
}

fn assert_effect(
    exact: &Game,
    actor: u16,
    id: ObjectId,
    effect: &IntroAuxiliaryEffect,
    update: usize,
) {
    assert_vector(exact, AUX, EFFECT_ORIGIN, effect.origin, update);
    assert_eq!(
        exact.memory.read_word(AUX + EFFECT_OWNER),
        if effect.owner == Some(id) {
            actor
        } else {
            VIEW
        }
    );
    let flags = exact.memory.read_byte(AUX + EFFECT_FLAGS);
    assert_eq!(flags & 128 != 0, effect.frozen);
    assert_eq!(flags & 64 != 0, effect.tracking);
    assert_eq!(flags & 63, 37); // unrelated source flags are preserved
    for (fields, values) in [
        (EFFECT_AXES, effect.axis_modes),
        (EFFECT_CONTROLS, effect.axis_controls),
    ] {
        for (field, value) in fields.into_iter().zip(values) {
            assert_eq!(exact.memory.read_byte(AUX + field), value, "field {field}");
        }
    }
    for (field, value) in [
        (0x6A90, effect.range as u16),
        (0x6C1C, effect.transition_mode),
        (0x6C24, effect.limit),
        (0x6C26, effect.remaining as u16),
    ] {
        assert_eq!(exact.memory.read_word(AUX + field), value, "field {field}");
    }
    assert_eq!(exact.memory.read_byte(AUX + 0x6C29), effect.target_axis);
    assert_eq!(exact.memory.read_byte(AUX + 0x6C28), effect.target_control);
}

#[test]
fn auxiliary_service_matches_original_including_frozen_owner_refresh_and_byte_doubling() {
    let rom = retail();
    for range in [0, 1, 127, 128, 255, 256, i16::MAX, i16::MIN, -1] {
        for flags in [37, 101, 165, 229] {
            for owned in [false, true] {
                let mut exact = Game::new(rom.clone()).unwrap();
                let actor = allocate(&mut exact.memory, 0).unwrap();
                let id = native_id();
                let mut effect = seed_effect(&mut exact, actor, id, flags, owned);
                let pose = IntroScenePose {
                    position: Vector3 {
                        x: i16::MIN,
                        y: 791,
                        z: i16::MAX,
                    },
                    ..Default::default()
                };
                write_vector(&mut exact, actor, POSITION, pose.position);
                exact.memory.write_word(0x3A, range as u16);
                exact.run_retail_oracle_routine(AUX_SERVICE, actor).unwrap();
                effect.configure_flyby(id, pose, range);
                assert_effect(&exact, actor, id, &effect, 0);
            }
        }
    }
}

fn cue_at(schedule: usize, update: usize) -> (OpeningCameraCue, u8) {
    use OpeningCameraCue::*;
    match schedule {
        0 => match update + 96 {
            0..182 => (Opening, 1),
            182..249 => (FirstCut, 2),
            249..293 => (SecondCut, 3),
            293..327 => (ThirdCut, 4),
            327..416 => (FourthCut, 5),
            _ => (FinalCut, 6),
        },
        1 => {
            if update == 0 {
                (FirstCut, 2)
            } else {
                (ThirdCut, 4)
            }
        }
        2 => (ThirdCut, 4), // missing the first gate must not skip it
        3 => match update {
            0..200 => (Opening, 1),
            200..300 => (FirstCut, 2),
            300 => (ThirdCut, 4),
            _ => (Opening, 1),
        },
        _ => unreachable!(),
    }
}

#[test]
fn original_dispatcher_transfers_zero_health_to_destruction_not_invisibility() {
    let (mut exact, actor) = authored_craft(&retail());
    for update in 0..=17 {
        exact
            .memory
            .write_byte(CUE, if update == 0 { 2 } else { 4 });
        exact.memory.write_word(CURRENT_OBJECT, actor);
        exact.run_retail_oracle_routine(UPDATE, actor).unwrap();
        exact.run_retail_oracle_routine(RESUME, actor).unwrap();
        if update == 16 {
            assert_eq!(exact.memory.read_byte(actor + 0x2D), 0);
            assert_eq!(exact.memory.read_byte(actor + 0x25) & 8, 0);
            assert_eq!(exact.memory.read_byte(actor + 0x23) & 2, 0);
            assert_eq!(
                exact.memory.read_word(actor + FIELD_SHAPE),
                SHAPE_BASE + 64 * SHAPE_STRIDE
            );
            assert_eq!(active_objects(&exact.memory), [actor]);
        } else if update == 17 {
            assert_eq!(exact.memory.read_byte(actor + 0x25) & 8, 8);
            let active = active_objects(&exact.memory);
            assert_eq!(active.len(), 3);
            let mut shapes: Vec<_> = active
                .iter()
                .copied()
                .filter(|id| *id != actor)
                .map(|id| (exact.memory.read_word(id + FIELD_SHAPE) - SHAPE_BASE) / SHAPE_STRIDE)
                .collect();
            shapes.sort_unstable();
            assert_eq!(shapes, [0, 12]);
            for id in active {
                assert_eq!(exact.memory.read_word(id + FIELD_STRATEGY), 0xA279);
                assert_eq!(exact.memory.read_byte(id + FIELD_STRATEGY + 2), 3);
            }
        }
    }
    exact.run_retail_oracle_routine(0x7F402D, actor).unwrap();
    let active = active_objects(&exact.memory);
    assert!(!active.contains(&actor));
    assert_eq!(active.len(), 2);
}

#[test]
fn complete_free_craft_path_matches_original_through_destruction_handoff() {
    let rom = retail();
    for schedule in 0..4 {
        for turned in [false, true] {
            for flags in [37, 101, 165, 229] {
                for owned in [false, true] {
                    let (mut exact, actor) = authored_craft(&rom);
                    let id = native_id();
                    let mut effect = seed_effect(&mut exact, actor, id, flags, owned);
                    let pose = IntroScenePose {
                        position: Vector3 {
                            x: 67,
                            y: -311,
                            z: 719,
                        },
                        rotation: if turned {
                            Rotation {
                                pitch: Angle::from_units(72),
                                yaw: Angle::from_units(199),
                                roll: Angle::from_units(53),
                            }
                        } else {
                            Rotation::default()
                        },
                    };
                    let mut native = OpeningFreeCraft::new(id, pose);
                    write_vector(&mut exact, actor, POSITION, pose.position);
                    for (field, angle) in ROTATION.into_iter().zip([
                        pose.rotation.pitch,
                        pose.rotation.yaw,
                        pose.rotation.roll,
                    ]) {
                        exact.memory.write_byte(actor + field, angle.units());
                    }
                    let updates = if schedule == 2 { 4_000 } else { 500 };
                    let mut audio_events = 0;
                    for update in 0..updates {
                        let (cue, encoded_cue) = cue_at(schedule, update);
                        exact.memory.write_byte(CUE, encoded_cue);
                        exact.memory.write_byte(MARKER, 0);
                        exact.memory.write_byte(MARKER_CLASS, 0);
                        exact.memory.write_word(CURRENT_OBJECT, actor);
                        exact.run_retail_oracle_routine(UPDATE, actor).unwrap();
                        exact.run_retail_oracle_routine(RESUME, actor).unwrap();
                        let event = native.tick(cue, &mut effect);
                        audio_events += usize::from(event.queue_departure_audio);
                        assert_vector(&exact, actor, POSITION, native.pose.position, update);
                        assert_vector(&exact, actor, VELOCITY, native.velocity, update);
                        assert_eq!(exact.memory.read_byte(actor + 0x18), 20);
                        for (field, angle) in ROTATION.into_iter().zip([
                            native.pose.rotation.pitch,
                            native.pose.rotation.yaw,
                            native.pose.rotation.roll,
                        ]) {
                            assert_eq!(exact.memory.read_byte(actor + field), angle.units());
                        }
                        assert_eq!(
                            exact.memory.read_byte(actor + 0x23) & 2 == 0,
                            native.is_visible(),
                            "visibility at update {update}"
                        );
                        assert_eq!(exact.memory.read_byte(actor + 9) & 1, 1);
                        assert_eq!(native.sort_depth_override(), Some(15_000));
                        assert_eq!(
                            exact.memory.read_word(actor + FIELD_SHAPE),
                            SHAPE_BASE + native.shape().catalog_index() as u16 * SHAPE_STRIDE
                        );
                        assert_eq!(
                            exact.memory.read_byte(MARKER),
                            if event.queue_departure_audio { 139 } else { 0 }
                        );
                        assert_eq!(
                            exact.memory.read_byte(MARKER_CLASS),
                            if event.queue_departure_audio { 2 } else { 0 }
                        );
                        assert_effect(&exact, actor, id, &effect, update);
                        let path = match native.phase() {
                            OpeningFreeCraftPhase::Initializing => unreachable!(),
                            OpeningFreeCraftPhase::AwaitingFirstCut => 0xFCD1,
                            OpeningFreeCraftPhase::AwaitingThirdCut => 0xFCD9,
                            OpeningFreeCraftPhase::Reappeared { .. } => 0xFCE6,
                            OpeningFreeCraftPhase::DeparturePause => 0xFCEF,
                            OpeningFreeCraftPhase::AwaitingDestruction => 0xFCF1,
                        };
                        assert_eq!(
                            exact.memory.read_word(actor + FIELD_PATH),
                            path,
                            "update {update}"
                        );
                        assert_eq!(
                            exact.memory.read_byte(actor + 0x2D) == 0,
                            event.request_destruction
                        );
                        if event.request_destruction {
                            assert_eq!(native.phase(), OpeningFreeCraftPhase::AwaitingDestruction);
                            assert_eq!(exact.memory.read_word(actor + FIELD_STRATEGY), 0x9DDE);
                            // The source actor-list dispatcher replaces this
                            // path with common destruction on the next update.
                            // Its debris/effect consumers are a separate gate.
                            let before = native;
                            assert_eq!(native.tick(cue, &mut effect), Default::default());
                            assert_eq!(native, before);
                            break;
                        }
                    }
                    assert_eq!(audio_events, usize::from(schedule != 2));
                }
            }
        }
    }
}
