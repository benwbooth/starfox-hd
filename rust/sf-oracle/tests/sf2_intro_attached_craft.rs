//! Native attached opening craft and effects, compared with the original
//! actor-list scheduler and source-created actors.

use sf2_game::intro_attached_craft::{
    OpeningAttachedCraftPhase, OpeningAttachedCraftSequence, OpeningBurstAudio, OpeningBurstSound,
    OpeningCraftStyle, OpeningDepartingCraftPhase,
};
use sf2_game::intro_destruction::{
    IntroDestructionContext, IntroExplosionAppearance, IntroExplosionVolume,
};
use sf2_game::intro_free_craft::IntroAuxiliaryEffect;
use sf2_game::intro_motion::{IntroAttachment, IntroScenePose};
use sf2_game::object::{
    active_objects, allocate, ACTIVE_LIST, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY,
    PLAYER_ONE, SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{
    Angle, Behavior, Object, ObjectKind, ObjectStore, RandomState, Rotation, ShapeId,
    StereoPosition, Vector3,
};

const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const CLEANUP: u32 = 0x7F402D;
const STRATEGY: u32 = 0x7F7E1E;
const VIEW: u16 = 0x033F;
const AUX: u16 = 0x0140;
const CRAFT_PATH: u16 = 0xFCF2;
const SHAPE_BASE: u16 = 0xBC9C;
const SHAPE_STRIDE: u16 = 28;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("attached-craft verification requires the user-owned retail SF2 ROM")
}

fn authored_family(rom: &[u8]) -> (Game, u16, u16) {
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
        .find(|actor| exact.memory.read_word(actor + FIELD_PATH) == CRAFT_PATH)
        .unwrap();
    exact.memory.write_word(ACTIVE_LIST, root);
    exact.memory.write_word(root, craft);
    exact.memory.write_word(root + 2, 0);
    exact.memory.write_word(craft, 0);
    exact.memory.write_word(craft + 2, root);
    exact.memory.write_word(root + 0x29, craft);
    exact.memory.write_word(craft + 0x27, 0);
    (exact, root, craft)
}

fn vector(exact: &Game, actor: u16, fields: [u16; 3]) -> Vector3 {
    let [x, y, z] = fields.map(|field| exact.memory.read_word(actor + field) as i16);
    Vector3 { x, y, z }
}

fn assert_vector(exact: &Game, actor: u16, fields: [u16; 3], expected: Vector3, update: usize) {
    assert_eq!(
        vector(exact, actor, fields),
        expected,
        "actor {actor}, update {update}, fields {fields:?}"
    );
}

fn assert_pose(exact: &Game, actor: u16, pose: IntroScenePose, update: usize) {
    assert_vector(exact, actor, [12, 14, 16], pose.position, update);
    assert_eq!(
        [18, 20, 22].map(|field| exact.memory.read_byte(actor + field)),
        [
            pose.rotation.pitch.units(),
            pose.rotation.yaw.units(),
            pose.rotation.roll.units()
        ],
        "rotation actor {actor}, update {update}"
    );
}

fn free_slots(exact: &Game) -> usize {
    let mut count = 0;
    let mut id = exact.memory.read_word(sf2_game::object::FREE_LIST);
    while id != 0 {
        count += 1;
        id = exact.memory.read_word(id);
    }
    count
}

#[test]
fn complete_attached_family_matches_original_including_deferred_death_births() {
    let rom = retail();
    for seed in [[0; 4], [255; 4], [1, 2, 3, 4], [17, 91, 211, 37]] {
        for clock_start in [0u8, 1, 4, 127, 255] {
            for rotating in [false, true] {
                for frozen in [false, true] {
                    for scrolling in [false, true] {
                        let (mut exact, root, craft) = authored_family(&rom);
                        let initial_free = free_slots(&exact);
                        let mut objects = ObjectStore::new();
                        let craft_id = objects
                            .allocate(Object::new(
                                ObjectKind::Effect,
                                ShapeId::from_catalog_index(64),
                                Behavior::Effect,
                            ))
                            .unwrap();
                        let departing_id = objects
                            .allocate(Object::new(
                                ObjectKind::Effect,
                                ShapeId::from_catalog_index(64),
                                Behavior::Effect,
                            ))
                            .unwrap();
                        let attachment = IntroAttachment {
                            offset: Vector3 {
                                x: -250,
                                y: -400,
                                z: -1500,
                            },
                            ..Default::default()
                        };
                        let mut native =
                            OpeningAttachedCraftSequence::new(craft_id, departing_id, attachment);
                        let mut random = RandomState::new(seed);
                        for (index, value) in seed.into_iter().enumerate() {
                            exact.memory.write_byte(0xE0 + index as u16, value);
                        }
                        let mut auxiliary = IntroAuxiliaryEffect {
                            frozen,
                            tracking: true,
                            ..Default::default()
                        };
                        exact
                            .memory
                            .write_byte(AUX + 0x6A8C, if frozen { 192 } else { 64 });
                        exact.memory.write_byte(0x1AA6, 2); // one audio listener
                        exact
                            .memory
                            .write_byte(AUX + 0x6AA0, if scrolling { 16 } else { 0 });
                        let mut flare_id = None;
                        let mut copy_id = None;
                        let mut burst_ids = Vec::new();
                        let mut burst_retired = Vec::new();
                        let mut explosion_ids = Vec::new();
                        let mut explosion_retired = Vec::new();
                        let mut craft_retired = false;
                        let mut copy_retired = false;
                        for update in 0..110 {
                            let before = active_objects(&exact.memory);
                            let available_slots = free_slots(&exact);
                            let mut parent = IntroScenePose {
                                position: Vector3 {
                                    x: (update as i16).wrapping_mul(719),
                                    y: -301,
                                    z: (update as i16).wrapping_mul(337),
                                },
                                rotation: if rotating {
                                    Rotation {
                                        pitch: Angle::from_units(update as u8),
                                        yaw: Angle::from_units((update as u8).wrapping_mul(3)),
                                        roll: Angle::from_units((update as u8).wrapping_mul(7)),
                                    }
                                } else {
                                    Rotation::default()
                                },
                            };
                            for (field, value) in [12, 14, 16].into_iter().zip([
                                parent.position.x,
                                parent.position.y,
                                parent.position.z,
                            ]) {
                                exact.memory.write_word(root + field, value as u16);
                            }
                            for (field, value) in [18, 20, 22].into_iter().zip([
                                parent.rotation.pitch,
                                parent.rotation.yaw,
                                parent.rotation.roll,
                            ]) {
                                exact.memory.write_byte(root + field, value.units());
                            }
                            parent.position.z = parent.position.z.wrapping_add(8);
                            let scene_phase = clock_start.wrapping_add(update as u8);
                            exact.memory.write_byte(0xC4, scene_phase.wrapping_sub(1));
                            exact.memory.write_byte(0x1D72, 1);
                            exact.memory.write_word(0x1E1C, 13);
                            exact.memory.write_word(0x1E20, 7);
                            exact.memory.write_word(0x1C31, 0);
                            exact.memory.write_word(0x1C35, 0);
                            exact.memory.write_word(0x1D16, 0);
                            let context = IntroDestructionContext {
                                available_slots,
                                compensate_scroll: scrolling,
                                scroll: Vector3 { x: 13, y: 0, z: 7 },
                                ..Default::default()
                            };
                            exact.memory.write_word(CURRENT_OBJECT, root);
                            exact.run_retail_oracle_routine(UPDATE, root).unwrap();
                            exact.run_retail_oracle_routine(RESUME, root).unwrap();
                            assert_eq!(
                                exact.memory.read_byte(0xC4),
                                scene_phase,
                                "scene phase update {update}"
                            );
                            let events = native
                                .tick(parent, scene_phase, &mut random, &mut auxiliary, &context)
                                .unwrap();
                            assert_pose(&exact, root, parent, update);
                            assert_eq!(
                                random.bytes(),
                                [0, 1, 2, 3].map(|i| exact.memory.read_byte(0xE0 + i)),
                                "random update {update}"
                            );
                            let active = active_objects(&exact.memory);
                            if events.split {
                                assert_eq!(update, 36);
                                flare_id = active
                                    .iter()
                                    .copied()
                                    .find(|id| exact.memory.read_word(id + FIELD_PATH) == 0xFD55);
                                copy_id = active
                                    .iter()
                                    .copied()
                                    .find(|id| exact.memory.read_word(id + FIELD_PATH) == 0xFD30);
                                assert!(flare_id.is_some() && copy_id.is_some());
                            }
                            for id in active.iter().copied().filter(|id| !before.contains(id)) {
                                if exact.memory.read_word(id + FIELD_PATH) == 0xC52B {
                                    burst_ids.push(id);
                                    burst_retired.push(false);
                                }
                            }
                            assert_eq!(
                                burst_ids.len(),
                                native.bursts().count(),
                                "burst births update {update}"
                            );
                            let mut new_explosions: Vec<_> = active
                                .iter()
                                .copied()
                                .filter(|id| {
                                    !before.contains(id)
                                        && exact.memory.read_word(id + FIELD_PATH) == 0
                                })
                                .collect();
                            for explosion in native.explosions().skip(explosion_ids.len()) {
                                let shape = SHAPE_BASE
                                    + explosion.shape().catalog_index() as u16 * SHAPE_STRIDE;
                                let index = new_explosions
                                    .iter()
                                    .position(|id| {
                                        exact.memory.read_word(id + FIELD_SHAPE) == shape
                                    })
                                    .unwrap();
                                explosion_ids.push(new_explosions.remove(index));
                                explosion_retired.push(false);
                            }
                            assert!(new_explosions.is_empty());
                            if !craft_retired {
                                assert_pose(&exact, craft, native.craft.pose, update);
                                assert_vector(
                                    &exact,
                                    craft,
                                    [0x1CCF, 0x1CD1, 0x1CD3],
                                    native.craft.attachment.offset,
                                    update,
                                );
                                assert_vector(
                                    &exact,
                                    craft,
                                    [0x32, 0x34, 0x36],
                                    native.craft.velocity,
                                    update,
                                );
                                let style = match native.craft.style {
                                    OpeningCraftStyle::Initial => 0,
                                    OpeningCraftStyle::AttachedDeparture => 5,
                                    _ => unreachable!(),
                                };
                                assert_eq!(exact.memory.read_byte(craft + 0x1CEF), style);
                                let (path, health) = match native.craft.phase {
                                    OpeningAttachedCraftPhase::WaitingForSplit { .. } => {
                                        (0xFCF7, 1)
                                    }
                                    OpeningAttachedCraftPhase::Holding { .. } => (0xFD14, 1),
                                    OpeningAttachedCraftPhase::Emitting { .. } => (0xFD19, 1),
                                    OpeningAttachedCraftPhase::AwaitingDestruction
                                    | OpeningAttachedCraftPhase::Finished => (0xFD21, 0),
                                };
                                assert_eq!(exact.memory.read_word(craft + FIELD_PATH), path);
                                assert_eq!(exact.memory.read_byte(craft + 0x2D), health);
                            }
                            if let (Some(copy), Some(id)) = (native.departing.as_ref(), copy_id) {
                                if !copy_retired {
                                    assert_pose(&exact, id, copy.pose, update);
                                    assert_vector(
                                        &exact,
                                        id,
                                        [0x32, 0x34, 0x36],
                                        copy.velocity,
                                        update,
                                    );
                                    assert_eq!(
                                        exact.memory.read_word(id + FIELD_SHAPE),
                                        SHAPE_BASE
                                            + copy.shape.catalog_index() as u16 * SHAPE_STRIDE
                                    );
                                    assert_eq!(exact.memory.read_byte(id + 0x1CEF), 4);
                                    assert_eq!(
                                        exact.memory.read_word(id + 0x1CC8),
                                        u16::from(copy.depth_offset())
                                    );
                                    assert_eq!(exact.memory.read_byte(id + 9) & 1, 1);
                                    let (path, health) = match copy.phase {
                                        OpeningDepartingCraftPhase::Waiting { .. } => (0xFD30, 1),
                                        OpeningDepartingCraftPhase::Drifting { .. } => (0xFD38, 1),
                                        OpeningDepartingCraftPhase::Emitting { .. } => (0xFD48, 1),
                                        OpeningDepartingCraftPhase::AwaitingDestruction
                                        | OpeningDepartingCraftPhase::Finished => (0xFD4C, 0),
                                    };
                                    assert_eq!(exact.memory.read_word(id + FIELD_PATH), path);
                                    assert_eq!(exact.memory.read_byte(id + 0x2D), health);
                                }
                            }
                            if let (Some(flare), Some(id)) = (native.flare.as_ref(), flare_id) {
                                if active.contains(&id) {
                                    assert_pose(&exact, id, flare.pose, update);
                                    assert_vector(
                                        &exact,
                                        id,
                                        [0x1CCF, 0x1CD1, 0x1CD3],
                                        flare.attachment.offset,
                                        update,
                                    );
                                    assert_eq!(
                                        [0x1CD5, 0x1CD6, 0x1CD7]
                                            .map(|f| exact.memory.read_byte(id + f)),
                                        [
                                            flare.attachment.rotation.pitch.units(),
                                            flare.attachment.rotation.yaw.units(),
                                            flare.attachment.rotation.roll.units()
                                        ]
                                    );
                                    assert_eq!(exact.memory.read_word(id + 6), root);
                                    assert_eq!(exact.memory.read_word(id + 0x1CD8), craft);
                                }
                            }
                            for ((burst, id), retired) in
                                native.bursts().zip(&burst_ids).zip(&burst_retired)
                            {
                                if !retired {
                                    assert_pose(&exact, *id, burst.pose, update);
                                    assert_eq!(
                                        exact.memory.read_word(id + FIELD_SHAPE),
                                        SHAPE_BASE
                                            + burst.shape.catalog_index() as u16 * SHAPE_STRIDE,
                                        "burst shape update {update}, phase {scene_phase}, id {id}"
                                    );
                                    assert_eq!(
                                        exact.memory.read_byte(id + 0x1CCA),
                                        128 | burst.color_frame
                                    );
                                    assert_eq!(
                                        exact.memory.read_byte(id + 0x1CDA),
                                        burst.size_bias()
                                    );
                                }
                            }
                            for (explosion, id) in native.explosions().zip(&explosion_ids) {
                                if !explosion.is_finished() {
                                    assert_vector(
                                        &exact,
                                        *id,
                                        [12, 14, 16],
                                        explosion.position,
                                        update,
                                    );
                                    assert_eq!(
                                        exact.memory.read_byte(id + 0x1CCA) & 127,
                                        explosion.color_frame,
                                        "explosion frame update {update}, id {id}"
                                    );
                                    match explosion.appearance {
                                        IntroExplosionAppearance::Sprite { size_bias, .. } => {
                                            assert_eq!(
                                                exact.memory.read_byte(id + 0x1CDA),
                                                size_bias
                                            )
                                        }
                                        IntroExplosionAppearance::Companion { channels } => {
                                            assert_eq!(
                                                [19, 21, 23]
                                                    .map(|f| exact.memory.read_byte(id + f)),
                                                channels
                                            )
                                        }
                                    }
                                }
                            }
                            assert_vector(
                                &exact,
                                AUX,
                                [0x6A92, 0x6A94, 0x6A96],
                                auxiliary.origin,
                                update,
                            );
                            let owner = if auxiliary.owner == Some(craft_id) {
                                craft
                            } else if auxiliary.owner == Some(departing_id) {
                                copy_id.unwrap()
                            } else {
                                0
                            };
                            assert_eq!(exact.memory.read_word(AUX + 0x6A98), owner);
                            assert_eq!(
                                [0x6A8D, 0x6A8E, 0x6A8F].map(|f| exact.memory.read_byte(AUX + f)),
                                auxiliary.axis_modes
                            );
                            assert_eq!(
                                exact.memory.read_word(AUX + 0x6A90) as i16,
                                auxiliary.range
                            );
                            assert_eq!(
                                exact.memory.read_word(AUX + 0x6C26) as i16,
                                auxiliary.remaining
                            );
                            assert_eq!(
                                exact.memory.read_byte(AUX + 0x6A8C) & 64 != 0,
                                auxiliary.tracking
                            );
                            let expected_marker =
                                match events.selected_audio.map(|audio| audio.sound) {
                                    None => 0,
                                    Some(OpeningBurstSound::Burst) => 112,
                                    Some(OpeningBurstSound::Departure) => 139,
                                };
                            assert_eq!(
                                exact.memory.read_word(0x1C31),
                                expected_marker,
                                "marker update {update}"
                            );
                            assert_eq!(
                                exact.memory.read_word(0x1C35),
                                if expected_marker == 0 { 0 } else { 2 }
                            );
                            let selected_count = usize::from(events.selected_audio.is_some());
                            assert_eq!(
                                exact.memory.read_word(0x1D16) as usize,
                                (events.explosion_audio.len() + selected_count) * 2,
                                "audio update {update}, queued {}",
                                exact.memory.read_word(0x1CF6)
                            );
                            if let Some(audio) = events.selected_audio {
                                let (volume, stereo) = audio.spatial(IntroScenePose::default());
                                let attenuation = match volume {
                                    IntroExplosionVolume::Near => 0,
                                    IntroExplosionVolume::Middle => 0x3000,
                                    IntroExplosionVolume::Far => 0x6000,
                                };
                                let pan = match stereo {
                                    StereoPosition::Center => 0,
                                    StereoPosition::Left => 0x1000,
                                    StereoPosition::Right => 0x2000,
                                };
                                assert_eq!(
                                    exact.memory.read_word(0x1CF6),
                                    expected_marker + attenuation + pan,
                                    "spatial sound update {update}"
                                );
                            }
                            for (index, sound) in events.explosion_audio.iter().enumerate() {
                                let expected = match sound {
                                    IntroExplosionVolume::Near => 112,
                                    IntroExplosionVolume::Middle => 12400,
                                    IntroExplosionVolume::Far => 24688,
                                };
                                assert_eq!(
                                    exact
                                        .memory
                                        .read_word(0x1CF6 + (index + selected_count) as u16 * 2),
                                    expected
                                );
                            }
                            exact.run_retail_oracle_routine(CLEANUP, root).unwrap();
                            let active = active_objects(&exact.memory);
                            if !craft_retired {
                                assert_eq!(active.contains(&craft), native.craft.is_visible());
                            }
                            if !copy_retired {
                                if let (Some(id), Some(copy)) = (copy_id, native.departing.as_ref())
                                {
                                    assert_eq!(active.contains(&id), !copy.is_finished());
                                }
                            }
                            if let (Some(id), Some(flare)) = (flare_id, native.flare.as_ref()) {
                                assert_eq!(active.contains(&id), flare.is_visible());
                            }
                            if events.attached_retired {
                                craft_retired = true;
                            }
                            if events.departing_retired {
                                copy_retired = true;
                            }
                            for ((burst, id), retired) in
                                native.bursts().zip(&burst_ids).zip(&mut burst_retired)
                            {
                                if !*retired {
                                    assert_eq!(active.contains(id), !burst.is_finished());
                                }
                                *retired = burst.is_finished();
                            }
                            for ((explosion, id), retired) in native
                                .explosions()
                                .zip(&explosion_ids)
                                .zip(&mut explosion_retired)
                            {
                                if !*retired {
                                    assert_eq!(active.contains(id), !explosion.is_finished());
                                }
                                *retired = explosion.is_finished();
                            }
                            let expected_count = 1
                                + usize::from(native.craft.is_visible())
                                + usize::from(
                                    native
                                        .departing
                                        .as_ref()
                                        .is_some_and(|copy| !copy.is_finished()),
                                )
                                + usize::from(
                                    native
                                        .flare
                                        .as_ref()
                                        .is_some_and(|flare| flare.is_visible()),
                                )
                                + native.bursts().filter(|b| !b.is_finished()).count()
                                + native.explosions().filter(|e| !e.is_finished()).count();
                            assert_eq!(active.len(), expected_count, "lifetimes update {update}");
                            let new_alive =
                                expected_count - 1 - usize::from(native.craft.is_visible());
                            assert_eq!(
                                free_slots(&exact),
                                initial_free + usize::from(craft_retired) - new_alive,
                                "free slots update {update}"
                            );
                            if native.is_finished() {
                                assert_eq!(update, 101);
                                assert_eq!(active, [root]);
                                break;
                            }
                        }
                        assert!(native.is_finished());
                    }
                }
            }
        }
    }
}

#[test]
fn selected_burst_audio_matches_original_distance_and_every_listener_yaw() {
    let mut exact = Game::new(retail()).unwrap();
    let actor = allocate(&mut exact.memory, 0).unwrap();
    exact.memory.write_word(PLAYER_ONE, VIEW);
    exact.memory.write_word(SELECTED_OBJECT, VIEW);
    // Test-only calling convention adapter in unused high WRAM. The original
    // selector, spatial sound routine, angle helper and GSU square root are
    // unchanged. These helpers return with RTS, so the adapter supplies JSR.
    const CALLER: u32 = 0x7FFE00;
    const WRAPPER: [u8; 7] = [0x20, 0xFB, 0xA3, 0x20, 0xAE, 0xA4, 0x6B];
    for (index, byte) in WRAPPER.into_iter().enumerate() {
        assert_eq!(exact.memory.read_long_byte(CALLER + index as u32), 0);
        exact.memory.write_long_byte(CALLER + index as u32, byte);
    }
    for (x, z) in [
        (0, 0),
        (1, 0),
        (-1, 0),
        (799, 0),
        (800, 0),
        (1299, 0),
        (1300, 0),
        (500, 500),
        (700, 700),
        (-500, 500),
        (500, -500),
        (-500, -500),
        (i16::MIN, i16::MAX),
    ] {
        for yaw in 0..=u8::MAX {
            let listener = IntroScenePose {
                position: Vector3 {
                    x: 311,
                    y: -900,
                    z: -701,
                },
                rotation: Rotation {
                    yaw: Angle::from_units(yaw),
                    ..Default::default()
                },
            };
            let source = Vector3 {
                x: listener.position.x.wrapping_add(x),
                y: 31000,
                z: listener.position.z.wrapping_add(z),
            };
            for (base, position) in [(VIEW, listener.position), (actor, source)] {
                for (field, value) in [12, 14, 16]
                    .into_iter()
                    .zip([position.x, position.y, position.z])
                {
                    exact.memory.write_word(base + field, value as u16);
                }
            }
            exact.memory.write_byte(VIEW + 21, yaw);
            exact.memory.write_word(0x1C31, 112);
            exact.memory.write_word(0x1C35, 2);
            exact.memory.write_word(0x1D16, 0);
            exact.run_retail_oracle_routine(CALLER, actor).unwrap();
            let (volume, stereo) = OpeningBurstAudio {
                sound: OpeningBurstSound::Burst,
                source,
            }
            .spatial(listener);
            let attenuation = match volume {
                IntroExplosionVolume::Near => 0,
                IntroExplosionVolume::Middle => 0x3000,
                IntroExplosionVolume::Far => 0x6000,
            };
            let pan = match stereo {
                StereoPosition::Center => 0,
                StereoPosition::Left => 0x1000,
                StereoPosition::Right => 0x2000,
            };
            assert_eq!(exact.memory.read_word(0x1D16), 2);
            assert_eq!(
                exact.memory.read_word(0x1CF6),
                112 + attenuation + pan,
                "x {x}, z {z}, yaw {yaw}"
            );
        }
    }
}
