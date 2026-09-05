//! The original actor-list update and cleanup drive both parent and child.
//! This checks scheduling and attachment behavior beyond isolated path tests.

use sf2_game::intro_logo::{
    LogoActorPhase, LogoClipping, LogoExitPolicy, LogoGlyph, LogoGlyphPair, LogoLayer,
    LogoOutlinePhase, LogoSceneScroll, NintendoLogoAnimation, NintendoLogoLayer,
    NintendoLogoOutline,
};
use sf2_game::object::{
    active_objects, allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY, PLAYER_ONE,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Angle, RandomState, Rotation, Vector3};

const ACTOR_LIST_UPDATE: u32 = 0x7F34E7;
const ACTOR_LIST_RESUME: u32 = 0x7F354A;
const ACTOR_LIST_CLEANUP: u32 = 0x7F402D;
const INITIAL_STRATEGY: u32 = 0x7F7E1E;
const LOGO_PATH: u16 = 0x93F0;
const CHILD_WAIT_PATH: u16 = 0xF022;
const CHILD_HOLD_PATH: u16 = 0xF028;
const POSITION_FIELDS: [u16; 3] = [0x0C, 0x0E, 0x10];
const ROTATION_FIELDS: [u16; 3] = [0x12, 0x14, 0x16];
const MATERIAL: u16 = 0x1CCD;
const CHILD_LINK: u16 = 0x29;
const PARENT_LINK: u16 = 6;
const SELECTED_PLAYER: u16 = 0x033F;
const SELECTED_AUX_SLOT: u16 = 0x0140;
const SELECTED_HORIZONTAL_POLICY: u16 = 0x6B77 + SELECTED_AUX_SLOT;
const RANDOM_START: u16 = 0xE0;
const RELEASE_FLAGS: u16 = 0xD77D;
const HORIZONTAL_SCROLL: u16 = 0x1E1C;
const DEPTH_SCROLL: u16 = 0x1E20;
const SHAPE_HEADER_BASE: u16 = 0xBC9C;
const SHAPE_HEADER_SIZE: u16 = 28;
const MAX_UPDATES: usize = 145;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("attachment tests require the user-owned retail SF2 ROM")
}

#[test]
fn complete_logo_family_matches_retail_scheduling_and_random_order() {
    const ASSEMBLY_PATH: u16 = 0x9284;
    const SWEEP_SHAPE: u16 = 0xC1DC;
    const PRIMARY_ROLE: u8 = 19;
    const SECONDARY_ROLE: u8 = 20;
    const ROLE: u16 = 0x2E;
    const EXPECTED_LAYERS: usize = 18;
    let rom = retail();
    for seed in [[0, 0, 0, 0], [71, 1, 255, 137], [255, 127, 128, 1]] {
        for scrolling in [false, true] {
            let origin = Vector3 {
                x: -901,
                y: 153,
                z: i16::MAX,
            };
            let mut native = NintendoLogoAnimation::new(origin);
            let mut random = RandomState::new(seed);
            let mut exact = Game::new(rom.clone()).unwrap();
            let assembly = allocate(&mut exact.memory, 0).unwrap();
            exact
                .memory
                .write_word(assembly + FIELD_PATH, ASSEMBLY_PATH);
            exact
                .memory
                .write_word(assembly + FIELD_STRATEGY, INITIAL_STRATEGY as u16);
            exact.memory.write_byte(
                assembly + FIELD_STRATEGY + 2,
                (INITIAL_STRATEGY >> 16) as u8,
            );
            exact.memory.write_byte(assembly + 0x2D, 1);
            for (field, value) in POSITION_FIELDS
                .into_iter()
                .zip([origin.x, origin.y, origin.z])
            {
                exact.memory.write_word(assembly + field, value as u16);
            }
            for (index, byte) in seed.into_iter().enumerate() {
                exact.memory.write_byte(RANDOM_START + index as u16, byte);
            }
            exact.memory.write_word(PLAYER_ONE, SELECTED_PLAYER);
            exact
                .memory
                .write_word(SELECTED_PLAYER + FIELD_PATH, SELECTED_AUX_SLOT);
            let mut glyphs = Vec::new();
            let mut sweep = None;
            for update in 0..MAX_UPDATES {
                let scroll = if scrolling {
                    LogoSceneScroll {
                        horizontal: -19,
                        depth: 10,
                        horizontal_locked: update % 2 == 0,
                    }
                } else {
                    LogoSceneScroll::default()
                };
                exact
                    .memory
                    .write_word(HORIZONTAL_SCROLL, scroll.horizontal as u16);
                exact.memory.write_word(DEPTH_SCROLL, scroll.depth as u16);
                exact.memory.write_byte(
                    SELECTED_HORIZONTAL_POLICY,
                    if scroll.horizontal_locked { 4 } else { 0 },
                );
                exact.memory.write_word(CURRENT_OBJECT, assembly);
                exact
                    .run_retail_oracle_routine(ACTOR_LIST_UPDATE, assembly)
                    .unwrap();
                exact
                    .run_retail_oracle_routine(ACTOR_LIST_RESUME, assembly)
                    .unwrap();
                native.tick(scroll, &mut random);
                let live = active_objects(&exact.memory);
                for object in live.iter().copied() {
                    let role = exact.memory.read_byte(object + ROLE);
                    if [PRIMARY_ROLE, SECONDARY_ROLE].contains(&role) && !glyphs.contains(&object) {
                        glyphs.push(object);
                    }
                    if exact.memory.read_word(object + FIELD_SHAPE) == SWEEP_SHAPE {
                        assert!(sweep.is_none() || sweep == Some(object));
                        sweep = Some(object);
                    }
                }
                glyphs.sort_unstable();
                assert_eq!(native.layers().count(), glyphs.len(), "update={update}");
                for (layer, object) in native.layers().zip(glyphs.iter().copied()) {
                    assert_pose(&exact, object, layer.actor.position, layer.actor.rotation);
                    assert_eq!(
                        exact.memory.read_byte(object + 0x23) & 2 == 0,
                        layer.actor.visible
                    );
                    assert_eq!(
                        exact.memory.read_byte(object + 0x1CC8),
                        layer.actor.depth_offset
                    );
                    assert_eq!(
                        exact.memory.read_byte(object + 0x1CDB),
                        layer.actor.texture_scroll_y
                    );
                    assert_eq!(
                        exact.memory.read_word(object + MATERIAL),
                        layer
                            .actor
                            .material_override
                            .map_or(0, |material| material.catalog_token())
                    );
                    let clipping = match layer.actor.clipping {
                        LogoClipping::PrimaryAssembly => 4,
                        LogoClipping::SecondaryAssembly => 5,
                        LogoClipping::Unclipped => 0,
                    };
                    assert_eq!(exact.memory.read_byte(object + 0x1CEF), clipping);
                    assert_eq!(
                        exact.memory.read_word(object + FIELD_SHAPE),
                        SHAPE_HEADER_BASE
                            + layer.actor.glyph.shape().catalog_index() as u16 * SHAPE_HEADER_SIZE
                    );
                    assert_eq!(
                        exact.memory.read_byte(object + ROLE),
                        match layer.actor.layer {
                            LogoLayer::Primary => PRIMARY_ROLE,
                            LogoLayer::Secondary => SECONDARY_ROLE,
                        }
                    );
                    assert_eq!(
                        exact.memory.read_byte(object + 0x25) & 8 != 0,
                        layer.actor.phase() == LogoActorPhase::Finished,
                        "update={update} object={object}"
                    );
                    if let Some(outline) = &layer.outline {
                        // The source retains the attachment link through End;
                        // cleanup is executed only after these pose checks.
                        if live.contains(&object) {
                            let child = exact.memory.read_word(object + CHILD_LINK);
                            assert_ne!(child, 0);
                            assert_pose(&exact, child, outline.position, outline.rotation);
                            assert_eq!(
                                exact.memory.read_word(child + MATERIAL),
                                outline.material.catalog_token()
                            );
                        }
                    }
                }
                assert_eq!(sweep.is_some(), native.sweep.is_some());
                if let (Some(object), Some(sweep)) = (sweep, native.sweep.as_ref()) {
                    assert_pose(&exact, object, sweep.position, sweep.rotation);
                }
                assert_eq!(
                    native.released(),
                    exact.memory.read_word(RELEASE_FLAGS) != 0
                );
                let mut actual_random = [0; 4];
                for (index, byte) in actual_random.iter_mut().enumerate() {
                    *byte = exact.memory.read_byte(RANDOM_START + index as u16);
                }
                assert_eq!(
                    random,
                    RandomState::new(actual_random),
                    "seed={seed:?} scroll={scrolling} update={update}"
                );
                exact
                    .run_retail_oracle_routine(ACTOR_LIST_CLEANUP, assembly)
                    .unwrap();
                let live = active_objects(&exact.memory);
                let expected = usize::from(!native.released())
                    + native
                        .layers()
                        .filter(|layer| layer.actor.phase() != LogoActorPhase::Finished)
                        .count()
                    + native
                        .layers()
                        .filter_map(|layer| layer.outline.as_ref())
                        .filter(|outline| outline.is_visible())
                        .count()
                    + usize::from(native.sweep.as_ref().is_some_and(|sweep| {
                        sweep.phase() != sf2_game::intro_logo::LogoSweepPhase::Finished
                    }));
                assert_eq!(live.len(), expected, "update={update}");
                if native.is_finished() {
                    assert_eq!(glyphs.len(), EXPECTED_LAYERS);
                    assert!(live.is_empty());
                    let before = native.clone();
                    let before_random = random;
                    native.tick(scroll, &mut random);
                    assert_eq!(native, before);
                    assert_eq!(random, before_random);
                    break;
                }
            }
            assert!(native.is_finished());
        }
    }
}

fn assert_pose(exact: &Game, object: u16, position: Vector3, rotation: Rotation) {
    for (field, value) in POSITION_FIELDS
        .into_iter()
        .zip([position.x, position.y, position.z])
    {
        assert_eq!(
            exact.memory.read_word(object + field) as i16,
            value,
            "position field={field}"
        );
    }
    for (field, value) in
        ROTATION_FIELDS
            .into_iter()
            .zip([rotation.pitch, rotation.yaw, rotation.roll])
    {
        assert_eq!(
            exact.memory.read_byte(object + field),
            value.units(),
            "rotation field={field}"
        );
    }
}

#[test]
fn outline_material_pose_and_removal_match_retail_actor_list() {
    let rom = retail();
    for release_update in [0, 31, 80] {
        for remove in [false, true] {
            for pitch in [0, 96, 248, 1] {
                let position = Vector3 {
                    x: i16::MAX,
                    y: i16::MIN,
                    z: -501,
                };
                let rotation = Rotation {
                    pitch: Angle::from_units(pitch),
                    yaw: Angle::from_units(71),
                    roll: Angle::from_units(37),
                };
                let pair = LogoGlyphPair {
                    glyph: LogoGlyph::Outline,
                    position,
                };
                let mut native = NintendoLogoLayer::new(pair, LogoLayer::Primary, rotation);
                native.actor.exit_policy = if remove {
                    LogoExitPolicy::Remove
                } else {
                    LogoExitPolicy::Disperse
                };
                let seed = [71, 1, 255, 137];
                let mut random = RandomState::new(seed);
                let mut exact = Game::new(rom.clone()).unwrap();
                let parent = allocate(&mut exact.memory, 0).unwrap();
                exact.memory.write_word(parent + FIELD_PATH, LOGO_PATH);
                exact.memory.write_word(
                    parent + FIELD_SHAPE,
                    SHAPE_HEADER_BASE
                        + pair.glyph.shape().catalog_index() as u16 * SHAPE_HEADER_SIZE,
                );
                exact
                    .memory
                    .write_word(parent + FIELD_STRATEGY, INITIAL_STRATEGY as u16);
                exact
                    .memory
                    .write_byte(parent + FIELD_STRATEGY + 2, (INITIAL_STRATEGY >> 16) as u8);
                exact.memory.write_byte(parent + 0x2D, 1);
                exact.memory.write_byte(parent + 0x2E, 19);
                exact.memory.write_byte(parent + 0x1CE2, u8::from(remove));
                for (field, value) in POSITION_FIELDS
                    .into_iter()
                    .zip([position.x, position.y, position.z])
                {
                    exact.memory.write_word(parent + field, value as u16);
                }
                for (field, value) in
                    ROTATION_FIELDS
                        .into_iter()
                        .zip([rotation.pitch, rotation.yaw, rotation.roll])
                {
                    exact.memory.write_byte(parent + field, value.units());
                }
                for (index, byte) in seed.into_iter().enumerate() {
                    exact.memory.write_byte(RANDOM_START + index as u16, byte);
                }
                exact.memory.write_word(PLAYER_ONE, SELECTED_PLAYER);
                exact
                    .memory
                    .write_word(SELECTED_PLAYER + FIELD_PATH, SELECTED_AUX_SLOT);
                let mut child = None;
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
                    exact.memory.write_word(CURRENT_OBJECT, parent);
                    exact
                        .run_retail_oracle_routine(ACTOR_LIST_UPDATE, parent)
                        .unwrap();
                    // The first pass only overlaps work while the graphics
                    // job is busy. The second resumes its saved list cursor.
                    exact
                        .run_retail_oracle_routine(ACTOR_LIST_RESUME, parent)
                        .unwrap();
                    let events = native.tick(update >= release_update, scroll, &mut random);
                    let child =
                        *child.get_or_insert_with(|| exact.memory.read_word(parent + CHILD_LINK));
                    assert_ne!(
                        child, 0,
                        "outline must spawn within its parent's first update"
                    );
                    assert_pose(&exact, parent, native.actor.position, native.actor.rotation);
                    let outline = native.outline.as_ref().unwrap();
                    assert_pose(&exact, child, outline.position, outline.rotation);
                    assert_eq!(
                        exact.memory.read_word(child + MATERIAL),
                        outline.material.catalog_token(),
                        "release={release_update} remove={remove} pitch={pitch} update={update}"
                    );
                    assert_eq!(
                        exact.memory.read_word(child + FIELD_SHAPE),
                        SHAPE_HEADER_BASE
                            + NintendoLogoOutline::SHAPE.catalog_index() as u16 * SHAPE_HEADER_SIZE
                    );
                    assert_eq!(exact.memory.read_word(child + PARENT_LINK), parent);
                    assert_eq!(
                        exact.memory.read_byte(child + 0x23) & 2,
                        0,
                        "child visibility is independent of parent"
                    );
                    for field in [0x1CC8, 0x1CDB, 0x1CEF] {
                        assert_eq!(
                            exact.memory.read_byte(child + field),
                            0,
                            "child must not inherit depth, texture scroll or clipping"
                        );
                    }
                    if !events.finished {
                        let path = match outline.phase() {
                            LogoOutlinePhase::InitialMaterial { .. } => CHILD_WAIT_PATH,
                            LogoOutlinePhase::Holding => CHILD_HOLD_PATH,
                            LogoOutlinePhase::Finished => panic!("live parent lost outline"),
                        };
                        assert_eq!(exact.memory.read_word(child + FIELD_PATH), path);
                    }
                    exact
                        .run_retail_oracle_routine(ACTOR_LIST_CLEANUP, parent)
                        .unwrap();
                    let live = active_objects(&exact.memory);
                    assert_eq!(
                        live.contains(&parent),
                        native.actor.phase() != LogoActorPhase::Finished
                    );
                    assert_eq!(live.contains(&child), outline.is_visible());
                    if events.finished {
                        assert!(
                            live.is_empty(),
                            "cleanup must remove the child with its parent"
                        );
                        let before = native;
                        native.tick(true, scroll, &mut random);
                        assert_eq!(native, before);
                        break;
                    }
                }
                assert_eq!(
                    native.actor.phase() == LogoActorPhase::Finished,
                    pitch % 8 == 0
                );
            }
        }
    }
}
