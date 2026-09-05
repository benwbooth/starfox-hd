//! Original common-death constructor, actor-list update and cleanup compared
//! with typed native effects. Includes the full source-authored craft lifetime.

use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_destruction::{
    IntroDestructionContext, IntroDestructionEffects, IntroExplosionActor,
    IntroExplosionAppearance, IntroExplosionPhase, IntroExplosionProfile, IntroExplosionVolume,
};
use sf2_game::intro_free_craft::{IntroAuxiliaryEffect, OpeningFreeCraftSequence};
use sf2_game::intro_motion::IntroScenePose;
use sf2_game::object::{
    active_objects, allocate, ACTIVE_LIST, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY,
    PLAYER_ONE, SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Behavior, Object, ObjectKind, ObjectStore, ShapeId, Vector3};

const DEATH: u32 = 0x03A055;
const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const CLEANUP: u32 = 0x7F402D;
const VIEW: u16 = 0x033F;
const SECOND_VIEW: u16 = 0x037E;
const AUX: u16 = 0x0140;
const POSITION: [u16; 3] = [12, 14, 16];
const SHAPE_BASE: u16 = 0xBC9C;
const SHAPE_STRIDE: u16 = 28;
const COLOR_FRAME: u16 = 0x1CCA;
const SIZE_BIAS: u16 = 0x1CDA;
const QUEUE: u16 = 0x1CF6;
const TAIL: u16 = 0x1D16;
const CUE: u16 = 0x1D72;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("destruction verification requires the user-owned retail SF2 ROM")
}

fn write_position(exact: &mut Game, object: u16, position: Vector3) {
    for (field, value) in POSITION
        .into_iter()
        .zip([position.x, position.y, position.z])
    {
        exact.memory.write_word(object + field, value as u16);
    }
}

fn setup_context(exact: &mut Game, context: &IntroDestructionContext) {
    exact.memory.write_word(PLAYER_ONE, VIEW);
    exact.memory.write_word(VIEW + FIELD_PATH, AUX);
    write_position(exact, VIEW, context.primary_listener);
    if let Some(listener) = context.secondary_listener {
        write_position(exact, SECOND_VIEW, listener);
    }
    exact.memory.write_byte(
        0x1AA6,
        if context.secondary_listener.is_some() {
            0
        } else {
            2
        },
    );
    exact.memory.write_byte(
        AUX + 0x6AA0,
        if context.compensate_scroll { 0x10 } else { 0 },
    );
    exact.memory.write_word(0x1E1C, context.scroll.x as u16);
    exact.memory.write_word(0x1E20, context.scroll.z as u16);
}

fn free_slots(exact: &Game) -> usize {
    let mut free_count: usize = 0;
    let mut free = exact.memory.read_word(sf2_game::object::FREE_LIST);
    while free != 0 {
        free_count += 1;
        free = exact.memory.read_word(free);
    }
    free_count
}

fn fill_pool(exact: &mut Game, parent: u16, available: usize) {
    // Reserve other scene occupants without executing them in this isolated
    // family. Count the real free list: excluded root children still own slots.
    let free_count = free_slots(exact);
    for _ in 0..free_count.saturating_sub(available) {
        allocate(&mut exact.memory, parent).unwrap();
    }
    exact.memory.write_word(ACTIVE_LIST, parent);
    exact.memory.write_word(parent, 0);
    exact.memory.write_word(parent + 2, 0);
}

fn assert_actor(exact: &Game, id: u16, native: &IntroExplosionActor, update: usize) {
    for (field, value) in
        POSITION
            .into_iter()
            .zip([native.position.x, native.position.y, native.position.z])
    {
        assert_eq!(
            exact.memory.read_word(id + field) as i16,
            value,
            "actor {id}, update {update}, field {field}"
        );
    }
    assert_eq!(
        exact.memory.read_word(id + FIELD_SHAPE),
        SHAPE_BASE + native.shape().catalog_index() as u16 * SHAPE_STRIDE
    );
    assert_eq!(
        exact.memory.read_byte(id + COLOR_FRAME) & 127,
        native.color_frame
    );
    assert_eq!(
        exact.memory.read_byte(id + 0x25) & 8 != 0,
        native.is_finished()
    );
    match native.appearance {
        IntroExplosionAppearance::Sprite { size_bias, .. } => {
            assert_eq!(exact.memory.read_byte(id + 0x20) & 16, 0);
            assert_eq!(exact.memory.read_byte(id + SIZE_BIAS), size_bias);
        }
        IntroExplosionAppearance::Companion { channels } => {
            assert_eq!(exact.memory.read_byte(id + 0x20) & 16, 16);
            for (field, value) in [19, 21, 23].into_iter().zip(channels) {
                assert_eq!(exact.memory.read_byte(id + field), value);
            }
        }
    }
    match native.phase() {
        IntroExplosionPhase::Animating { age, limit } => {
            assert_eq!(exact.memory.read_byte(id + 10), age);
            assert_eq!(exact.memory.read_byte(id + 11), limit);
        }
        IntroExplosionPhase::AwaitingDestruction => {
            assert_eq!(exact.memory.read_byte(id + 10), 1);
            assert_eq!(exact.memory.read_byte(id + 11), 2);
            assert_eq!(exact.memory.read_byte(id + 0x2D), 0);
        }
        IntroExplosionPhase::Finished => {}
    }
}

fn assert_audio(exact: &Game, previous_tail: u16, audio: &[IntroExplosionVolume]) {
    let mut tail = previous_tail;
    for volume in audio {
        let marker = match volume {
            IntroExplosionVolume::Near => 0x70,
            IntroExplosionVolume::Middle => 0x3070,
            IntroExplosionVolume::Far => 0x6070,
        };
        assert_eq!(exact.memory.read_word(QUEUE + tail), marker);
        tail = (tail + 2) & 31;
    }
    assert_eq!(exact.memory.read_word(TAIL), tail);
}

#[test]
fn every_signed_axis_distance_matches_original_explosion_audio_attenuation() {
    let mut exact = Game::new(retail()).unwrap();
    let parent = allocate(&mut exact.memory, 0).unwrap();
    let listener = Vector3 {
        x: i16::MAX,
        y: i16::MIN,
        z: -311,
    };
    write_position(&mut exact, VIEW, listener);
    exact.memory.write_word(TAIL, 30);
    for offset in i16::MIN..=i16::MAX {
        let position = Vector3 {
            x: listener.x.wrapping_add(offset),
            y: i16::MAX,
            z: listener.z,
        };
        write_position(&mut exact, parent, position);
        let tail = exact.memory.read_word(TAIL);
        exact.run_retail_oracle_routine(0x068000, parent).unwrap();
        assert_audio(
            &exact,
            tail,
            &[IntroExplosionVolume::between(position, listener)],
        );
    }
}

#[test]
fn source_capacity_failure_enters_diagnostics_and_native_surfaces_an_error() {
    let rom = retail();
    // No-free-slot branch enters a console service before returning failure;
    // running this branch in the headless oracle reaches its cycle guard.
    assert_eq!(
        &rom[0x12925..0x12933],
        &[0xC2, 0x20, 0x9B, 0xAE, 0xAA, 0x12, 0xD0, 0x06, 0xE2, 0x20, 0xBB, 0x4C, 0x69, 0x29,]
    );
    assert_eq!(
        &rom[0x12969..0x12979],
        &[
            0xA5, 0x5E, 0x29, 0xE7, 0x85, 0x5E, 0x8F, 0x3A, 0x30, 0x00, 0x22, 0x32, 0x80, 0x00,
            0x18, 0x6B,
        ]
    );
    for (shape, required_slots) in [(0, 1), (64, 2)] {
        let profile = IntroExplosionProfile::for_shape(ShapeId::from_catalog_index(shape)).unwrap();
        for available_slots in 0..required_slots {
            let error = IntroDestructionEffects::spawn(
                profile,
                Vector3::default(),
                &IntroDestructionContext {
                    available_slots,
                    ..Default::default()
                },
            )
            .unwrap_err();
            assert_eq!(error.required_slots, required_slots);
            assert_eq!(error.available_slots, available_slots);
        }
    }
}

#[test]
fn every_catalog_shape_matches_original_destruction_profile_and_allocation_order() {
    let rom = retail();
    for shape in 0..sf2_data::shape_data::SHAPE_DATA_COUNT {
        let mut exact = Game::new(rom.clone()).unwrap();
        let parent = allocate(&mut exact.memory, 0).unwrap();
        exact.memory.write_word(
            parent + FIELD_SHAPE,
            SHAPE_BASE + shape as u16 * SHAPE_STRIDE,
        );
        let position = Vector3 {
            x: 701,
            y: -313,
            z: 997,
        };
        write_position(&mut exact, parent, position);
        let context = IntroDestructionContext {
            available_slots: 2,
            ..Default::default()
        };
        setup_context(&mut exact, &context);
        let before = active_objects(&exact.memory);
        exact.run_retail_oracle_routine(DEATH, parent).unwrap();
        let profile =
            IntroExplosionProfile::for_shape(ShapeId::from_catalog_index(shape as u16)).unwrap();
        let (native, audio) = IntroDestructionEffects::spawn(profile, position, &context).unwrap();
        let mut spawned: Vec<_> = active_objects(&exact.memory)
            .into_iter()
            .filter(|id| !before.contains(id))
            .collect();
        spawned.sort_unstable();
        assert_eq!(spawned.len(), native.actors().count(), "shape {shape}");
        for (id, actor) in spawned.into_iter().zip(native.actors()) {
            assert_actor(&exact, id, actor, 0);
        }
        assert_audio(&exact, 0, &audio);
        assert_eq!(exact.memory.read_byte(parent + 0x25) & 8, 8);
    }
}

#[test]
fn full_effect_families_match_original_birth_animation_scroll_and_cleanup() {
    let rom = retail();
    for shape in [0, 9, 10, 11, 12, 64, 89, 338] {
        for available in 1..=3 {
            for suppressed in [false, true] {
                for scrolling in [false, true] {
                    let mut exact = Game::new(rom.clone()).unwrap();
                    let parent = allocate(&mut exact.memory, 0).unwrap();
                    exact
                        .memory
                        .write_word(parent + FIELD_SHAPE, SHAPE_BASE + shape * SHAPE_STRIDE);
                    exact
                        .memory
                        .write_byte(parent + 0x25, if suppressed { 2 } else { 0 });
                    // The normal path initializer has already consumed the
                    // constructor's one-update exemption from death handling.
                    exact.memory.write_byte(parent + 0x31, 0x10);
                    let position = Vector3 {
                        x: i16::MAX,
                        y: -617,
                        z: i16::MIN,
                    };
                    write_position(&mut exact, parent, position);
                    fill_pool(&mut exact, parent, available);
                    let mut context = IntroDestructionContext {
                        available_slots: available,
                        suppress_effects: suppressed,
                        primary_listener: Vector3 {
                            x: 117,
                            y: 10_001,
                            z: -1_119,
                        },
                        secondary_listener: Some(Vector3 {
                            x: i16::MIN,
                            y: -10_001,
                            z: i16::MAX,
                        }),
                        compensate_scroll: scrolling,
                        ..Default::default()
                    };
                    let profile =
                        IntroExplosionProfile::for_shape(ShapeId::from_catalog_index(shape))
                            .unwrap();
                    let spawned = IntroDestructionEffects::spawn(profile, position, &context);
                    if spawned.is_err() {
                        assert_eq!(available, 1);
                        assert!(!suppressed);
                        continue; // source diagnostic halt is checked separately
                    }
                    let (mut native, audio) = spawned.unwrap();
                    let mut ids = Vec::new();
                    for update in 0..80 {
                        if update > 0 {
                            context.available_slots = available + 1
                                - native.actors().filter(|actor| !actor.is_finished()).count();
                        }
                        assert_eq!(free_slots(&exact), context.available_slots);
                        context.scroll = Vector3 {
                            x: (update as i16).wrapping_mul(977),
                            y: 3_333,
                            z: (update as i16).wrapping_mul(-337),
                        };
                        setup_context(&mut exact, &context);
                        let tail = exact.memory.read_word(TAIL);
                        let previous_active = active_objects(&exact.memory);
                        exact.memory.write_word(CURRENT_OBJECT, parent);
                        exact.run_retail_oracle_routine(UPDATE, parent).unwrap();
                        exact.run_retail_oracle_routine(RESUME, parent).unwrap_or_else(|error|
                            panic!("shape={shape} slots={available} suppressed={suppressed} scrolling={scrolling} update={update}: {error:?}"));
                        let mut expected_audio = if update == 0 {
                            audio.clone()
                        } else {
                            Vec::new()
                        };
                        expected_audio.extend(native.tick(&context).unwrap());
                        let mut newborn: Vec<_> = active_objects(&exact.memory)
                            .into_iter()
                            .filter(|id| !previous_active.contains(id))
                            .collect();
                        newborn.sort_unstable();
                        ids.extend(newborn);
                        assert_eq!(ids.len(), native.actors().count());
                        assert_audio(&exact, tail, &expected_audio);
                        let active = active_objects(&exact.memory);
                        for (&id, actor) in ids.iter().zip(native.actors()) {
                            if active.contains(&id) {
                                assert_actor(&exact, id, actor, update);
                            } else {
                                assert!(actor.is_finished());
                            }
                        }
                        exact.run_retail_oracle_routine(CLEANUP, parent).unwrap();
                        let active = active_objects(&exact.memory);
                        if update == 0 {
                            assert!(!active.contains(&parent));
                        }
                        for (&id, actor) in ids.iter().zip(native.actors()) {
                            assert_eq!(!active.contains(&id), actor.is_finished());
                        }
                        if native.is_finished() {
                            break;
                        }
                    }
                    assert!(native.is_finished());
                    let before = native.clone();
                    assert!(native.tick(&context).unwrap().is_empty());
                    assert_eq!(native, before);
                }
            }
        }
    }
}

fn authored_free_craft(rom: &[u8]) -> (Game, u16) {
    let mut exact = Game::new(rom.to_vec()).unwrap();
    let root = allocate(&mut exact.memory, 0).unwrap();
    exact.memory.write_word(root + FIELD_PATH, 0xFA11);
    exact.memory.write_word(root + FIELD_STRATEGY, 0x7E1E);
    exact.memory.write_byte(root + FIELD_STRATEGY + 2, 0x7F);
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
        .find(|id| exact.memory.read_word(id + FIELD_PATH) == 0xFCC5)
        .unwrap();
    exact.memory.write_word(ACTIVE_LIST, craft);
    exact.memory.write_word(craft, 0);
    exact.memory.write_word(craft + 2, 0);
    (exact, craft)
}

#[test]
fn complete_free_craft_sequence_matches_original_through_final_effect_removal() {
    let rom = retail();
    for available in 2..=3 {
        for scrolling in [false, true] {
            for early_cuts in [false, true] {
                let (mut exact, craft) = authored_free_craft(&rom);
                fill_pool(&mut exact, craft, available);
                let id = ObjectStore::new()
                    .allocate(Object::new(
                        ObjectKind::Effect,
                        ShapeId::from_catalog_index(64),
                        Behavior::Effect,
                    ))
                    .unwrap();
                let mut native = OpeningFreeCraftSequence::new(id, IntroScenePose::default());
                let mut auxiliary = IntroAuxiliaryEffect::default();
                let mut context = IntroDestructionContext {
                    available_slots: available,
                    compensate_scroll: scrolling,
                    ..Default::default()
                };
                let mut effects = Vec::new();
                let mut departure_audio = 0;
                let mut explosion_audio = 0;
                let mut craft_retired = false;
                for update in 0..400 {
                    context.available_slots = available + usize::from(native.craft_has_retired())
                        - native
                            .effects()
                            .filter(|actor| !actor.is_finished())
                            .count();
                    assert_eq!(free_slots(&exact), context.available_slots);
                    let cue = if early_cuts {
                        if update == 0 {
                            OpeningCameraCue::FirstCut
                        } else {
                            OpeningCameraCue::ThirdCut
                        }
                    } else if update < 86 {
                        OpeningCameraCue::Opening
                    } else if update < 197 {
                        OpeningCameraCue::FirstCut
                    } else {
                        OpeningCameraCue::ThirdCut
                    };
                    exact.memory.write_byte(
                        CUE,
                        match cue {
                            OpeningCameraCue::Opening => 1,
                            OpeningCameraCue::FirstCut => 2,
                            _ => 4,
                        },
                    );
                    context.scroll = Vector3 {
                        x: (update as i16).wrapping_mul(13),
                        y: 0,
                        z: (update as i16).wrapping_mul(-11),
                    };
                    setup_context(&mut exact, &context);
                    let tail = exact.memory.read_word(TAIL);
                    let previous_active = active_objects(&exact.memory);
                    exact.memory.write_word(CURRENT_OBJECT, craft);
                    exact.run_retail_oracle_routine(UPDATE, craft).unwrap();
                    exact.run_retail_oracle_routine(RESUME, craft).unwrap_or_else(|error|
                        panic!("slots={available} scrolling={scrolling} early={early_cuts} update={update}: {error:?}"));
                    let events = native.tick(cue, &mut auxiliary, &context).unwrap();
                    departure_audio += usize::from(events.queue_departure_audio);
                    explosion_audio += events.explosion_audio.len();
                    if !events.queue_departure_audio {
                        assert_audio(&exact, tail, &events.explosion_audio);
                    }
                    let active = active_objects(&exact.memory);
                    let mut newborn: Vec<_> = active
                        .iter()
                        .copied()
                        .filter(|id| !previous_active.contains(id))
                        .collect();
                    newborn.sort_unstable();
                    effects.extend(newborn);
                    assert_eq!(effects.len(), native.effects().count(), "update {update}");
                    for (&id, actor) in effects.iter().zip(native.effects()) {
                        if active.contains(&id) {
                            assert_actor(&exact, id, actor, update);
                        } else {
                            assert!(actor.is_finished());
                        }
                    }
                    if !craft_retired && exact.memory.read_byte(craft + 0x25) & 8 != 0 {
                        craft_retired = true;
                    }
                    exact.run_retail_oracle_routine(CLEANUP, craft).unwrap();
                    let active = active_objects(&exact.memory);
                    assert_eq!(
                        !craft_retired
                            && active.contains(&craft)
                            && exact.memory.read_byte(craft + 0x23) & 2 == 0,
                        native.craft_is_visible(),
                        "update {update}"
                    );
                    for (&id, actor) in effects.iter().zip(native.effects()) {
                        assert_eq!(!active.contains(&id), actor.is_finished());
                    }
                    if native.is_finished() {
                        break;
                    }
                }
                assert!(native.is_finished());
                assert_eq!(departure_audio, 1);
                assert_eq!(explosion_audio, 2);
                let before = native.clone();
                let auxiliary_before = auxiliary;
                assert_eq!(
                    native
                        .tick(OpeningCameraCue::Opening, &mut auxiliary, &context)
                        .unwrap(),
                    Default::default()
                );
                assert_eq!(native, before);
                assert_eq!(auxiliary, auxiliary_before);
            }
        }
    }
}
