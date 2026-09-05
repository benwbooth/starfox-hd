//! Execute unmodified retail logo paths, including their dispatch, loop,
//! allocation and yielding handlers. Source addresses remain oracle-only.

use sf2_game::intro_logo::{LogoArrivalPhase, NintendoLogoArrival, NintendoLogoAssembly};
use sf2_game::object::{
    active_objects, allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z,
    FIELD_SHAPE, FIELD_X, FIELD_Y, FIELD_Z,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, Rotation, Vector3};

const DISPATCH: u32 = 0x7F7E53;
const ASSEMBLY_PATH: u16 = 0x9284;
const GLYPH_PATH: u16 = 0x93F0;
const APPROACH_START: u16 = 0x943D;
const APPROACH_BODY: u16 = 0x943F;
const SETTLING_PATH: u16 = 0x9445;
const HOLD_PATH: u16 = 0x9451;
const SWEEP_PATH: u16 = 0x9378;
const RELEASE_FLAGS: u16 = 0xD77D;
const CHILD_ROLE: u16 = 0x2E;
const APPROACH_SPEED: u16 = 0x27;
const FIXED_POSE_FLAGS: u16 = 0x21;
const FIRST_LAYER: u8 = 19;
const SECOND_LAYER: u8 = 20;
const SHAPE_HEADER_BASE: u16 = 0xBC9C;
const SHAPE_HEADER_SIZE: u16 = 28;
const SWEEP_SHAPE: u16 = 0xC1DC;
const GLYPH_COUNT: usize = 9;
const ARRIVAL_TEST_UPDATES: usize = 64;
const ASSEMBLY_RELEASE_UPDATE: usize = 100;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("logo differential tests require the user-owned retail SF2 ROM")
}

fn seed_position(exact: &mut Game, object: u16, position: Vector3) {
    for (field, value) in [FIELD_X, FIELD_Y, FIELD_Z]
        .into_iter()
        .zip([position.x, position.y, position.z])
    {
        exact.memory.write_word(object + field, value as u16);
    }
}

fn position(exact: &Game, object: u16) -> Vector3 {
    Vector3 {
        x: exact.memory.read_word(object + FIELD_X) as i16,
        y: exact.memory.read_word(object + FIELD_Y) as i16,
        z: exact.memory.read_word(object + FIELD_Z) as i16,
    }
}

fn tick(exact: &mut Game, object: u16) {
    exact.memory.write_word(CURRENT_OBJECT, object);
    exact.run_retail_oracle_routine(DISPATCH, object).unwrap();
}

#[test]
fn all_starting_pitches_match_retail_arrival_and_settling() {
    let rom = retail();
    const YAW: u8 = 71;
    const ROLL: u8 = 37;
    const DEPTH_STEP: u8 = 50;
    // This depth crosses the signed boundary during the approach. Nonzero
    // pitch residues also prove that we do not invent a snap-to-zero rule.
    let origin = Vector3 {
        x: i16::MIN,
        y: i16::MAX,
        z: i16::MAX - i16::from(DEPTH_STEP),
    };
    for pitch in u8::MIN..=u8::MAX {
        let rotation = Rotation {
            pitch: Angle::from_units(pitch),
            yaw: Angle::from_units(YAW),
            roll: Angle::from_units(ROLL),
        };
        let mut native = NintendoLogoArrival::new(origin, rotation);
        let mut exact = Game::new(rom.clone()).unwrap();
        let object = allocate(&mut exact.memory, 0).unwrap();
        seed_position(&mut exact, object, origin);
        exact.memory.write_word(object + FIELD_PATH, APPROACH_START);
        exact.memory.write_byte(object + APPROACH_SPEED, DEPTH_STEP);
        exact.memory.write_byte(object + FIXED_POSE_FLAGS, 1);
        exact.memory.write_byte(object + CHILD_ROLE, FIRST_LAYER);
        for (field, value) in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z]
            .into_iter()
            .zip([pitch, YAW, ROLL])
        {
            exact.memory.write_byte(object + field, value);
        }
        for update in 0..ARRIVAL_TEST_UPDATES {
            native.tick();
            tick(&mut exact, object);
            assert_eq!(
                native.position,
                position(&exact, object),
                "pitch={pitch} update={update}"
            );
            for (field, value) in [FIELD_ROT_X, FIELD_ROT_Y, FIELD_ROT_Z].into_iter().zip([
                native.rotation.pitch,
                native.rotation.yaw,
                native.rotation.roll,
            ]) {
                assert_eq!(
                    exact.memory.read_byte(object + field),
                    value.units(),
                    "pitch={pitch} update={update} field={field}"
                );
            }
            let expected_path = match native.phase() {
                LogoArrivalPhase::Approaching { .. } => APPROACH_BODY,
                LogoArrivalPhase::Settling => SETTLING_PATH,
                LogoArrivalPhase::Holding => HOLD_PATH,
            };
            assert_eq!(
                exact.memory.read_word(object + FIELD_PATH),
                expected_path,
                "pitch={pitch} update={update}"
            );
        }
    }
}

#[test]
fn complete_parent_path_matches_native_spawns_spacing_and_release() {
    let rom = retail();
    for x in [i16::MIN, -1, 0, i16::MAX] {
        let origin = Vector3 { x, y: -71, z: 911 };
        let mut native = NintendoLogoAssembly::new(origin);
        let mut exact = Game::new(rom.clone()).unwrap();
        let parent = allocate(&mut exact.memory, 0).unwrap();
        seed_position(&mut exact, parent, origin);
        exact.memory.write_word(parent + FIELD_PATH, ASSEMBLY_PATH);
        let mut pairs = Vec::new();
        let mut sweep = None;
        for update in 0..=ASSEMBLY_RELEASE_UPDATE {
            let events = native.tick();
            tick(&mut exact, parent);
            if let Some(pair) = events.glyph_pair {
                pairs.push(pair);
            }
            if let Some(position) = events.sweep_position {
                assert!(sweep.replace(position).is_none(), "duplicate sweep");
                assert_eq!(pairs.len(), GLYPH_COUNT);
            }
            let children = active_objects(&exact.memory);
            let mut glyphs: Vec<_> = children
                .iter()
                .copied()
                .filter(|object| exact.memory.read_word(object + FIELD_PATH) == GLYPH_PATH)
                .collect();
            // Slots increase in allocation order, while the active list is
            // reverse insertion order. Neither depends on captured poses.
            glyphs.sort_unstable();
            assert_eq!(glyphs.len(), pairs.len() * 2, "update={update}");
            for (objects, pair) in glyphs.chunks_exact(2).zip(&pairs) {
                for (object, role) in objects.iter().copied().zip([FIRST_LAYER, SECOND_LAYER]) {
                    assert_eq!(position(&exact, object), pair.position, "update={update}");
                    assert_eq!(exact.memory.read_byte(object + CHILD_ROLE), role);
                    assert_eq!(
                        exact.memory.read_word(object + FIELD_SHAPE),
                        SHAPE_HEADER_BASE
                            + pair.glyph.shape().catalog_index() as u16 * SHAPE_HEADER_SIZE
                    );
                }
            }
            let sweeps: Vec<_> = children
                .into_iter()
                .filter(|object| exact.memory.read_word(object + FIELD_PATH) == SWEEP_PATH)
                .collect();
            assert_eq!(sweeps.len(), usize::from(sweep.is_some()));
            if let Some(expected) = sweep {
                assert_eq!(position(&exact, sweeps[0]), expected);
                assert_eq!(exact.memory.read_word(sweeps[0] + FIELD_SHAPE), SWEEP_SHAPE);
            }
            assert_eq!(
                events.release,
                exact.memory.read_word(RELEASE_FLAGS) != 0,
                "update={update}"
            );
            assert_eq!(events.release, update == ASSEMBLY_RELEASE_UPDATE);
        }
        // A completed native controller must not emit repeated release events.
        assert_eq!(native.tick(), Default::default());
    }
}
