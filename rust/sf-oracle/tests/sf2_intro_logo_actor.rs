//! Whole retail logo-actor paths versus typed native state. No production
//! code executes these source bytes; no path handlers are patched here.

use sf2_game::intro_logo::{
    LogoActorEvents, LogoActorPhase, LogoArrivalPhase, LogoDrawStyle, LogoExitPolicy, LogoGlyph,
    LogoGlyphPair, LogoLayer, LogoSceneScroll, LogoSweepPhase, NintendoLogoActor,
    NintendoLogoSweep,
};
use sf2_game::object::{
    active_objects, allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, PLAYER_ONE,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, RandomState, Rotation, Vector3};

const DISPATCH: u32 = 0x7F7E53;
const INITIAL_PATH: u16 = 0x93F0;
const APPROACH_PATH: u16 = 0x943F;
const SETTLING_PATH: u16 = 0x9445;
const HOLD_PATH: u16 = 0x9451;
const DISPERSAL_PATH: u16 = 0x9484;
const END_PATH: u16 = 0x948B;
const OUTLINE_CHILD_PATH: u16 = 0xF01D;
const POSITION_FIELDS: [u16; 3] = [0x0C, 0x0E, 0x10];
const ROTATION_FIELDS: [u16; 3] = [0x12, 0x14, 0x16];
const VELOCITY_FIELDS: [u16; 3] = [0x32, 0x34, 0x36];
const CHILD_ROLE: u16 = 0x2E;
const VISIBILITY_FLAGS: u16 = 0x23;
const INVISIBLE: u8 = 2;
const LIFETIME_FLAGS: u16 = 0x25;
const FINISHED: u8 = 8;
const RELEASE_FLAGS: u16 = 0xD77D;
const PRIMARY_ROLE: u8 = 19;
const SECONDARY_ROLE: u8 = 20;
const DEPTH_OFFSET: u16 = 0x1CC8;
const MATERIAL: u16 = 0x1CCD;
const TEXTURE_SCROLL: u16 = 0x1CDB;
const EXIT_POLICY: u16 = 0x1CE2;
const DRAW_STYLE: u16 = 0x1CEF;
const RANDOM_START: u16 = 0xE0;
const SELECTED_PLAYER: u16 = 0x033F;
const SELECTED_AUX_SLOT: u16 = 0x0140;
const SELECTED_HORIZONTAL_POLICY: u16 = 0x6B77 + SELECTED_AUX_SLOT;
const HORIZONTAL_SCROLL: u16 = 0x1E1C;
const DEPTH_SCROLL: u16 = 0x1E20;
const SHAPE_HEADER_BASE: u16 = 0xBC9C;
const SHAPE_HEADER_SIZE: u16 = 28;
const MAX_UPDATES: usize = 130;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("actor differential tests require the user-owned retail SF2 ROM")
}

fn assert_vector(exact: &Game, object: u16, fields: [u16; 3], expected: Vector3) {
    for (field, value) in fields.into_iter().zip([expected.x, expected.y, expected.z]) {
        assert_eq!(
            exact.memory.read_word(object + field) as i16,
            value,
            "field={field}"
        );
    }
}

#[test]
fn sweep_delay_traversal_and_release_match_the_complete_retail_path() {
    const SWEEP_PATH: u16 = 0x9378;
    const WAIT_PATH: u16 = 0x9389;
    const TRAVERSE_PATH: u16 = 0x938D;
    const SWEEP_HOLD_PATH: u16 = 0x9392;
    const SWEEP_END_PATH: u16 = 0x939A;
    let rom = retail();
    for release_update in [0, 19, 31, 32, 80] {
        for x in [i16::MIN, 0, i16::MAX] {
            let origin = Vector3 {
                x,
                y: -71,
                z: i16::MAX,
            };
            let mut native = NintendoLogoSweep::new(origin);
            let mut exact = Game::new(rom.clone()).unwrap();
            let object = allocate(&mut exact.memory, 0).unwrap();
            exact.memory.write_word(object + FIELD_PATH, SWEEP_PATH);
            exact.memory.write_word(PLAYER_ONE, SELECTED_PLAYER);
            exact
                .memory
                .write_word(SELECTED_PLAYER + FIELD_PATH, SELECTED_AUX_SLOT);
            for (field, value) in POSITION_FIELDS
                .into_iter()
                .zip([origin.x, origin.y, origin.z])
            {
                exact.memory.write_word(object + field, value as u16);
            }
            for update in 0..MAX_UPDATES {
                let scroll = LogoSceneScroll {
                    horizontal: -19,
                    depth: 10,
                    horizontal_locked: update % 2 == 0,
                };
                exact
                    .memory
                    .write_word(HORIZONTAL_SCROLL, scroll.horizontal as u16);
                exact.memory.write_word(DEPTH_SCROLL, scroll.depth as u16);
                exact.memory.write_byte(
                    SELECTED_HORIZONTAL_POLICY,
                    if scroll.horizontal_locked { 4 } else { 0 },
                );
                exact
                    .memory
                    .write_word(RELEASE_FLAGS, u16::from(update >= release_update));
                exact.memory.write_word(CURRENT_OBJECT, object);
                exact.run_retail_oracle_routine(DISPATCH, object).unwrap();
                let finished = native.tick(update >= release_update, scroll);
                assert_vector(&exact, object, POSITION_FIELDS, native.position);
                for (field, value) in ROTATION_FIELDS.into_iter().zip([
                    native.rotation.pitch,
                    native.rotation.yaw,
                    native.rotation.roll,
                ]) {
                    assert_eq!(exact.memory.read_byte(object + field), value.units());
                }
                let source_path = match native.phase() {
                    LogoSweepPhase::Delayed { .. } => WAIT_PATH,
                    LogoSweepPhase::Traversing { .. } => TRAVERSE_PATH,
                    LogoSweepPhase::Holding => SWEEP_HOLD_PATH,
                    LogoSweepPhase::Finished => SWEEP_END_PATH,
                };
                assert_eq!(exact.memory.read_word(object + FIELD_PATH), source_path);
                assert_eq!(
                    finished,
                    exact.memory.read_byte(object + LIFETIME_FLAGS) & FINISHED != 0
                );
                if finished {
                    let before = native;
                    assert!(!native.tick(true, scroll));
                    assert_eq!(native, before);
                    break;
                }
            }
            assert_eq!(native.phase(), LogoSweepPhase::Finished);
        }
    }
}

#[test]
fn complete_actor_lifetimes_match_retail_across_layers_release_and_random_seeds() {
    let rom = retail();
    for seed in [0u8, 1, 63, 127, 128, 254, 255] {
        for layer in [LogoLayer::Primary, LogoLayer::Secondary] {
            for glyph in [LogoGlyph::CapitalN, LogoGlyph::Outline] {
                for release_update in [0, 31, 80] {
                    for pitch in [0, 96, 248, 1] {
                        for remove in [false, true] {
                            let seed_bytes = [
                                seed,
                                seed.wrapping_add(71),
                                seed.wrapping_add(127),
                                seed.wrapping_mul(3),
                            ];
                            let pair = LogoGlyphPair {
                                glyph,
                                position: Vector3 {
                                    x: i16::MAX,
                                    y: i16::MIN,
                                    z: i16::MAX,
                                },
                            };
                            let rotation = Rotation {
                                pitch: Angle::from_units(pitch),
                                yaw: Angle::from_units(71),
                                roll: Angle::from_units(37),
                            };
                            let mut native = NintendoLogoActor::new(pair, layer, rotation);
                            native.exit_policy = if remove {
                                LogoExitPolicy::Remove
                            } else {
                                LogoExitPolicy::Disperse
                            };
                            let mut random = RandomState::new(seed_bytes);
                            let mut exact = Game::new(rom.clone()).unwrap();
                            let object = allocate(&mut exact.memory, 0).unwrap();
                            exact.memory.write_word(object + FIELD_PATH, INITIAL_PATH);
                            exact.memory.write_word(
                                object + FIELD_SHAPE,
                                SHAPE_HEADER_BASE
                                    + glyph.shape().catalog_index() as u16 * SHAPE_HEADER_SIZE,
                            );
                            exact.memory.write_byte(
                                object + CHILD_ROLE,
                                if layer == LogoLayer::Primary {
                                    PRIMARY_ROLE
                                } else {
                                    SECONDARY_ROLE
                                },
                            );
                            exact
                                .memory
                                .write_byte(object + EXIT_POLICY, u8::from(remove));
                            for (field, value) in POSITION_FIELDS.into_iter().zip([
                                pair.position.x,
                                pair.position.y,
                                pair.position.z,
                            ]) {
                                exact.memory.write_word(object + field, value as u16);
                            }
                            for (field, value) in ROTATION_FIELDS.into_iter().zip([
                                rotation.pitch,
                                rotation.yaw,
                                rotation.roll,
                            ]) {
                                exact.memory.write_byte(object + field, value.units());
                            }
                            for (index, value) in seed_bytes.into_iter().enumerate() {
                                exact.memory.write_byte(RANDOM_START + index as u16, value);
                            }
                            exact.memory.write_word(PLAYER_ONE, SELECTED_PLAYER);
                            exact
                                .memory
                                .write_word(SELECTED_PLAYER + FIELD_PATH, SELECTED_AUX_SLOT);
                            for update in 0..MAX_UPDATES {
                                let scroll = LogoSceneScroll {
                                    horizontal: -19,
                                    depth: 10,
                                    horizontal_locked: update % 2 == 0,
                                };
                                exact
                                    .memory
                                    .write_word(HORIZONTAL_SCROLL, scroll.horizontal as u16);
                                exact.memory.write_word(DEPTH_SCROLL, scroll.depth as u16);
                                exact.memory.write_byte(
                                    SELECTED_HORIZONTAL_POLICY,
                                    if scroll.horizontal_locked { 4 } else { 0 },
                                );
                                exact
                                    .memory
                                    .write_word(RELEASE_FLAGS, u16::from(update >= release_update));
                                exact.memory.write_word(CURRENT_OBJECT, object);
                                exact.run_retail_oracle_routine(DISPATCH, object).unwrap();
                                let events =
                                    native.tick(update >= release_update, scroll, &mut random);
                                assert_vector(&exact, object, POSITION_FIELDS, native.position);
                                assert_vector(&exact, object, VELOCITY_FIELDS, native.velocity);
                                for (field, value) in ROTATION_FIELDS.into_iter().zip([
                                    native.rotation.pitch,
                                    native.rotation.yaw,
                                    native.rotation.roll,
                                ]) {
                                    assert_eq!(exact.memory.read_byte(object + field), value.units(), "seed={seed} layer={layer:?} glyph={glyph:?} release={release_update} pitch={pitch} remove={remove} update={update} rotation={field}");
                                }
                                assert_eq!(
                                    native.visible,
                                    exact.memory.read_byte(object + VISIBILITY_FLAGS) & INVISIBLE
                                        == 0
                                );
                                assert_eq!(
                                    native.depth_offset,
                                    exact.memory.read_byte(object + DEPTH_OFFSET)
                                );
                                assert_eq!(
                                    native
                                        .material_override
                                        .map_or(0, |material| material.catalog_token()),
                                    exact.memory.read_word(object + MATERIAL)
                                );
                                assert_eq!(
                                    native.texture_scroll_y,
                                    exact.memory.read_byte(object + TEXTURE_SCROLL)
                                );
                                let source_style = match native.draw_style {
                                    LogoDrawStyle::PrimaryAssembly => 4,
                                    LogoDrawStyle::SecondaryAssembly => 5,
                                    LogoDrawStyle::Normal => 0,
                                };
                                assert_eq!(
                                    source_style,
                                    exact.memory.read_byte(object + DRAW_STYLE)
                                );
                                let source_path = match native.phase() {
                                    LogoActorPhase::Arriving(LogoArrivalPhase::Approaching {
                                        ..
                                    }) => APPROACH_PATH,
                                    LogoActorPhase::Arriving(LogoArrivalPhase::Settling) => {
                                        SETTLING_PATH
                                    }
                                    LogoActorPhase::Arriving(LogoArrivalPhase::Holding) => {
                                        HOLD_PATH
                                    }
                                    LogoActorPhase::Dispersing { .. } => DISPERSAL_PATH,
                                    LogoActorPhase::Finished => END_PATH,
                                };
                                assert_eq!(
                                    source_path,
                                    exact.memory.read_word(object + FIELD_PATH)
                                );
                                for (index, value) in random.bytes().into_iter().enumerate() {
                                    assert_eq!(
                                        value,
                                        exact.memory.read_byte(RANDOM_START + index as u16)
                                    );
                                }
                                let has_child =
                                    layer == LogoLayer::Primary && glyph == LogoGlyph::Outline;
                                assert_eq!(events.spawn_outline_child, has_child && update == 0);
                                assert_eq!(
                                    active_objects(&exact.memory)
                                        .into_iter()
                                        .filter(|child| exact.memory.read_word(child + FIELD_PATH)
                                            == OUTLINE_CHILD_PATH)
                                        .count(),
                                    usize::from(has_child)
                                );
                                assert_eq!(
                                    events.finished,
                                    exact.memory.read_byte(object + LIFETIME_FLAGS) & FINISHED != 0
                                );
                                if events.finished {
                                    let before = native;
                                    let previous_random = random;
                                    assert_eq!(
                                        native.tick(true, scroll, &mut random),
                                        LogoActorEvents::default()
                                    );
                                    assert_eq!(native, before);
                                    assert_eq!(random, previous_random);
                                    break;
                                }
                            }
                            if pitch % 8 == 0 {
                                assert_eq!(native.phase(), LogoActorPhase::Finished);
                            } else {
                                assert_eq!(
                                    native.phase(),
                                    LogoActorPhase::Arriving(LogoArrivalPhase::Settling)
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
