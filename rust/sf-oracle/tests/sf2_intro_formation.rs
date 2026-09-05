//! Original formation construction, path execution and arithmetic compared
//! with native typed state. Machine-specific state is restricted to this test.

use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_formation::{
    advance_formation_impulse, chase_formation_angle, opening_formation_placement,
    OpeningFormationAudio, OpeningFormationCraft, OpeningFormationPhase, OpeningFormationShot,
};
use sf2_game::intro_free_craft::IntroAuxiliaryEffect;
use sf2_game::intro_motion::IntroScenePose;
use sf2_game::intro_root::OpeningFormationMember;
use sf2_game::object::{
    active_objects, allocate, ACTIVE_LIST, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY,
    PLAYER_ONE, SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{
    Angle, Behavior, Object, ObjectKind, ObjectStore, Rotation, ShapeId, StereoPosition, Vector3,
};
use sf_oracle::{call, Entry, SnesBus};

const UPDATE: u32 = 0x7F34E7;
const RESUME: u32 = 0x7F354A;
const STRATEGY: u32 = 0x7F7E1E;
const PATH: u16 = 0xFBD0;
const VIEW: u16 = 0x033F;
const AUX: u16 = 0x0140;
const SHAPE_BASE: u16 = 0xBC9C;
const SHAPE_STRIDE: u16 = 28;
const CUE: u16 = 0x1D72;
const POSITION: [u16; 3] = [0x0C, 0x0E, 0x10];
const IMPULSE: [u16; 3] = [0x1CCF, 0x1CD1, 0x1CD3];
const VELOCITY: [u16; 3] = [0x32, 0x34, 0x36];
const ROTATION: [u16; 3] = [0x12, 0x14, 0x16];
const MEMBERS: [OpeningFormationMember; 3] = [
    OpeningFormationMember::First,
    OpeningFormationMember::Second,
    OpeningFormationMember::Third,
];

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("formation verification requires the user-owned retail SF2 ROM")
}

fn bus() -> SnesBus {
    let rom = retail();
    let mut bus = SnesBus::new(rom.clone());
    for (source, destination, length) in
        [(0x010000, 0x7F0000, 0x7E00), (0x050000, 0x7F7E00, 0x4E00)]
    {
        for offset in 0..length {
            bus.write8(destination + offset as u32, rom[source + offset]);
        }
    }
    bus
}

#[test]
fn every_coarse_heading_pair_matches_original_shortest_arc_easing() {
    let mut bus = bus();
    for current in u8::MIN..=u8::MAX {
        for target in u8::MIN..=u8::MAX {
            bus.write8(0x7E003A, current);
            call(
                &mut bus,
                0x7F27B5,
                &Entry {
                    a: u16::from(target),
                    p: 0x20,
                    dbr: 0x7E,
                    ..Default::default()
                },
            );
            assert_eq!(
                chase_formation_angle(Angle::from_units(current), Angle::from_units(target))
                    .units(),
                bus.read8(0x7E003A),
                "current={current} target={target}"
            );
        }
    }
}

#[test]
fn combined_impulse_helper_preserves_add_then_decay_and_signed_wrap() {
    const ACTOR: u16 = 0x03BD;
    let mut bus = bus();
    for value in i16::MIN..=i16::MAX {
        let mut pose = IntroScenePose {
            position: Vector3 {
                x: i16::MAX,
                y: i16::MIN,
                z: -111,
            },
            rotation: Rotation {
                pitch: Angle::from_units(37),
                yaw: Angle::from_units(211),
                roll: Angle::from_units(value as u8),
            },
        };
        let mut impulse = Vector3 {
            x: value,
            y: value.wrapping_neg(),
            z: value.wrapping_mul(3),
        };
        for (fields, vector) in [(POSITION, pose.position), (IMPULSE, impulse)] {
            for (field, component) in fields.into_iter().zip([vector.x, vector.y, vector.z]) {
                bus.write16(0x7E0000 + u32::from(ACTOR + field), component as u16);
            }
        }
        for (field, angle) in
            ROTATION
                .into_iter()
                .zip([pose.rotation.pitch, pose.rotation.yaw, pose.rotation.roll])
        {
            bus.write8(0x7E0000 + u32::from(ACTOR + field), angle.units());
        }
        call(
            &mut bus,
            0x06FA04,
            &Entry {
                x: ACTOR,
                p: 0x20,
                dbr: 0x7E,
                ..Default::default()
            },
        );
        advance_formation_impulse(&mut pose, &mut impulse);
        for (fields, vector) in [(POSITION, pose.position), (IMPULSE, impulse)] {
            for (field, component) in fields.into_iter().zip([vector.x, vector.y, vector.z]) {
                assert_eq!(
                    bus.read16(0x7E0000 + u32::from(ACTOR + field)) as i16,
                    component,
                    "input={value} field={field}"
                );
            }
        }
        for (field, angle) in
            ROTATION
                .into_iter()
                .zip([pose.rotation.pitch, pose.rotation.yaw, pose.rotation.roll])
        {
            assert_eq!(
                bus.read8(0x7E0000 + u32::from(ACTOR + field)),
                angle.units()
            );
        }
    }
}

#[test]
fn all_authored_formation_placements_match_original_tables() {
    let rom = retail();
    for (shot_index, shot) in [
        OpeningFormationShot::Arrival,
        OpeningFormationShot::Pursuit,
        OpeningFormationShot::Reappearance,
        OpeningFormationShot::Exit,
    ]
    .into_iter()
    .enumerate()
    {
        for (member_index, member) in MEMBERS.into_iter().enumerate() {
            let index = shot_index * MEMBERS.len() + member_index;
            let placement = opening_formation_placement(member, shot);
            for (table, component) in [
                (0x3FB83, placement.position.x),
                (0x3FB9B, placement.position.y),
                (0x3FBB3, placement.position.z),
            ] {
                assert_eq!(
                    i16::from_le_bytes(
                        rom[table + index * 2..table + index * 2 + 2]
                            .try_into()
                            .unwrap()
                    ),
                    component
                );
            }
            for (table, component) in [
                (0x3FBCB, placement.pitch.units()),
                (0x3FBD7, placement.yaw.units()),
                (0x3FBE3, placement.duration),
            ] {
                assert_eq!(rom[table + index], component);
            }
        }
    }
}

fn authored_craft(rom: &[u8], member_index: usize) -> (Game, u16) {
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
        exact.run_retail_oracle_routine(STRATEGY, root).unwrap();
    }
    let craft = active_objects(&exact.memory)
        .into_iter()
        .find(|actor| {
            exact.memory.read_word(actor + FIELD_PATH) == PATH
                && exact.memory.read_byte(actor + 0x2E) == member_index as u8
        })
        .unwrap();
    assert_eq!(
        exact.memory.read_word(craft + FIELD_SHAPE),
        SHAPE_BASE + 89 * SHAPE_STRIDE
    );
    // Keep original construction flags, but isolate this member from siblings
    // whose separate scripts and allocation events are tested independently.
    exact.memory.write_word(ACTIVE_LIST, craft);
    exact.memory.write_word(craft, 0);
    exact.memory.write_word(craft + 2, 0);
    (exact, craft)
}

fn vector(exact: &Game, actor: u16, fields: [u16; 3]) -> Vector3 {
    let [x, y, z] = fields.map(|field| exact.memory.read_word(actor + field) as i16);
    Vector3 { x, y, z }
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
        1 => match update {
            0..100 => (Opening, 1),
            100..250 => (FirstCut, 2),
            250..300 => (SecondCut, 3),
            300..400 => (ThirdCut, 4),
            _ => (FourthCut, 5),
        },
        2 => (ThirdCut, 4),
        _ => unreachable!(),
    }
}

#[test]
fn source_created_members_match_full_path_through_end_or_destruction_handoff() {
    let rom = retail();
    for (member_index, member) in MEMBERS.into_iter().enumerate() {
        for schedule in 0..3 {
            for roll in [0, 127, 128, 255] {
                for candidates in 0..5 {
                    let (mut exact, actor) = authored_craft(&rom, member_index);
                    let mut objects = ObjectStore::new();
                    let id = objects
                        .allocate(Object::new(
                            ObjectKind::Effect,
                            ShapeId::from_catalog_index(89),
                            Behavior::Effect,
                        ))
                        .unwrap();
                    let pose = IntroScenePose {
                        position: vector(&exact, actor, POSITION),
                        rotation: Rotation {
                            roll: Angle::from_units(roll),
                            ..Default::default()
                        },
                    };
                    exact.memory.write_byte(actor + 0x16, roll);
                    let mut native = OpeningFormationCraft::new(id, member, pose);
                    let mut auxiliary = IntroAuxiliaryEffect::default();
                    let mut mapping = Vec::new();
                    if candidates != 0 {
                        for shape in [338, 64, 338] {
                            let target_id = objects
                                .allocate(Object::new(
                                    ObjectKind::Effect,
                                    ShapeId::from_catalog_index(shape),
                                    Behavior::Effect,
                                ))
                                .unwrap();
                            let target = allocate(&mut exact.memory, actor).unwrap();
                            exact.memory.write_byte(target + 0x2D, 1);
                            exact.memory.write_word(
                                target + FIELD_SHAPE,
                                SHAPE_BASE + shape * SHAPE_STRIDE,
                            );
                            mapping.push((target_id, target));
                        }
                    }
                    for update in 0..500 {
                        for (index, &(target_id, target)) in mapping.iter().enumerate() {
                            let position = match candidates {
                                1 => Vector3 {
                                    x: (index as i16 * 311) - 1200 + update as i16 * 7,
                                    y: update as i16 * -13,
                                    z: 2900,
                                },
                                2 => Vector3 {
                                    x: 900,
                                    y: index as i16 * 1301,
                                    z: 2200,
                                }, // equal X/Z distance; first active wins
                                3 => Vector3 {
                                    x: 14000,
                                    y: 0,
                                    z: 14000,
                                }, // outside search radius
                                4 if update > 115 => Vector3 {
                                    x: i16::MAX.wrapping_add(update as i16 * 3),
                                    y: i16::MIN.wrapping_sub(update as i16 * 17),
                                    z: i16::MIN.wrapping_add(update as i16),
                                },
                                4 => Vector3 {
                                    x: 600,
                                    y: -1000,
                                    z: 2100,
                                },
                                _ => unreachable!(),
                            };
                            objects.get_mut(target_id).unwrap().base.position = position;
                            for (field, component) in POSITION
                                .into_iter()
                                .zip([position.x, position.y, position.z])
                            {
                                exact.memory.write_word(target + field, component as u16);
                            }
                        }
                        let (cue, encoded) = cue_at(schedule, update);
                        exact.memory.write_byte(CUE, encoded);
                        exact.memory.write_byte(0x1C31, 0);
                        exact.memory.write_word(0x1D16, 0);
                        exact.memory.write_word(CURRENT_OBJECT, actor);
                        exact.run_retail_oracle_routine(UPDATE, actor).unwrap();
                        exact.run_retail_oracle_routine(RESUME, actor).unwrap();
                        let events = native.tick(cue, &objects, &mut auxiliary);
                        let context = format!("member={member:?} schedule={schedule} roll={roll} candidates={candidates} update={update} phase={:?} source={:04x}", native.phase(), exact.memory.read_word(actor + FIELD_PATH));
                        let target = native.tracked_actor.map_or(0, |id| {
                            mapping
                                .iter()
                                .find(|(native_id, _)| *native_id == id)
                                .unwrap()
                                .1
                        });
                        assert_eq!(
                            exact.memory.read_word(actor + 6),
                            target,
                            "tracking identity {context}"
                        );
                        assert_eq!(
                            vector(&exact, actor, POSITION),
                            native.pose.position,
                            "position {context}"
                        );
                        assert_eq!(
                            vector(&exact, actor, VELOCITY),
                            native.velocity,
                            "velocity {context}"
                        );
                        assert_eq!(
                            vector(&exact, actor, IMPULSE),
                            native.impulse,
                            "impulse {context}"
                        );
                        for (field, angle) in ROTATION.into_iter().zip([
                            native.pose.rotation.pitch,
                            native.pose.rotation.yaw,
                            native.pose.rotation.roll,
                        ]) {
                            assert_eq!(
                                exact.memory.read_byte(actor + field),
                                angle.units(),
                                "rotation {field} {context}"
                            );
                        }
                        assert_eq!(
                            exact.memory.read_byte(actor + 0x18),
                            native.speed,
                            "speed {context}"
                        );
                        assert_eq!(
                            exact.memory.read_word(actor + 0x1CE4),
                            native.elapsed_updates,
                            "elapsed {context}"
                        );
                        assert_eq!(
                            exact.memory.read_byte(actor + 0x1CCC) != 0,
                            native.trail_enabled,
                            "trail {context}"
                        );
                        assert_eq!(
                            exact.memory.read_byte(actor + 0x23) & 2 != 0,
                            !native.is_visible()
                                && native.phase() != OpeningFormationPhase::Finished,
                            "visibility {context}"
                        );
                        assert_eq!(
                            exact.memory.read_byte(0x1C31),
                            if events.pursuit_audio.is_some() {
                                171
                            } else {
                                0
                            },
                            "audio {context}"
                        );
                        if let Some(audio) = events.pursuit_audio {
                            let spatial = audio.spatial(IntroScenePose::default());
                            assert_eq!(
                                exact.memory.read_word(0x1D16),
                                if spatial.is_some() { 2 } else { 0 },
                                "audio count {context}"
                            );
                            if let Some(stereo) = spatial {
                                assert_eq!(
                                    exact.memory.read_word(0x1CF6),
                                    171 + encoded_pan(stereo),
                                    "audio payload {context}"
                                );
                            }
                        }
                        assert_eq!(
                            exact.memory.read_byte(actor + 0x2D) == 0,
                            events.request_destruction,
                            "death {context}"
                        );
                        assert_eq!(
                            exact.memory.read_byte(actor + 0x25) & 8 != 0,
                            events.finished,
                            "end {context}"
                        );
                        let path = match native.phase() {
                            OpeningFormationPhase::Initializing => unreachable!(),
                            OpeningFormationPhase::InitialWait { .. } => 0xFBE0,
                            OpeningFormationPhase::InitialHold { .. } => 0xFBE9,
                            OpeningFormationPhase::ArrivalBank { .. } => 0xFBF0,
                            OpeningFormationPhase::AwaitingFirstCut => 0xFBF4,
                            OpeningFormationPhase::FirstCutPause => 0xFBFD,
                            OpeningFormationPhase::PursuitBank { .. } => 0xFC0A,
                            OpeningFormationPhase::TrackingBank { .. } => 0xFC13,
                            OpeningFormationPhase::Tracking { .. } => 0xFC1E,
                            OpeningFormationPhase::Climbing { .. } => 0xFC24,
                            OpeningFormationPhase::AwaitingSecondCut => 0xFC27,
                            OpeningFormationPhase::AwaitingThirdCut => 0xFC33,
                            OpeningFormationPhase::Reappeared { .. } => 0xFC4F,
                            OpeningFormationPhase::DepartureBank { .. }
                            | OpeningFormationPhase::AwaitingDestruction => 0xFC5F,
                            OpeningFormationPhase::AwaitingFourthCut => 0xFC63,
                            OpeningFormationPhase::Exiting { .. } => 0xFC75,
                            OpeningFormationPhase::Finished => 0xFC76,
                        };
                        assert_eq!(
                            exact.memory.read_word(actor + FIELD_PATH),
                            path,
                            "path {context}"
                        );
                        if events.request_destruction {
                            assert_eq!(
                                vector(&exact, AUX, [0x6A92, 0x6A94, 0x6A96]),
                                auxiliary.origin,
                                "auxiliary origin {context}"
                            );
                            assert_eq!(exact.memory.read_word(AUX + 0x6A98), actor);
                            assert_eq!(auxiliary.owner, Some(id));
                            assert_eq!(
                                [0x6A8D, 0x6A8E, 0x6A8F]
                                    .map(|field| exact.memory.read_byte(AUX + field)),
                                auxiliary.axis_modes
                            );
                            assert_eq!(
                                exact.memory.read_word(AUX + 0x6A90) as i16,
                                auxiliary.range
                            );
                        }
                        if events.request_destruction || events.finished {
                            break;
                        }
                    }
                    if schedule != 2 {
                        assert!(matches!(
                            native.phase(),
                            OpeningFormationPhase::Finished
                                | OpeningFormationPhase::AwaitingDestruction
                        ));
                    }
                }
            }
        }
    }
}

fn encoded_pan(stereo: StereoPosition) -> u16 {
    match stereo {
        StereoPosition::Center => 0,
        StereoPosition::Left => 0x1000,
        StereoPosition::Right => 0x2000,
    }
}

#[test]
fn pursuit_audio_matches_fixed_range_source_service_and_all_listener_headings() {
    let mut exact = Game::new(retail()).unwrap();
    let actor = allocate(&mut exact.memory, 0).unwrap();
    exact.memory.write_word(PLAYER_ONE, VIEW);
    exact.memory.write_word(SELECTED_OBJECT, VIEW);
    // Calling-convention adapter only: invoke original listener selection and
    // fixed-range sound routines with a normal subroutine return frame.
    const CALLER: u32 = 0x7FFE00;
    const WRAPPER: [u8; 7] = [0x20, 0xFB, 0xA3, 0x20, 0x2A, 0xA5, 0x6B];
    for (index, byte) in WRAPPER.into_iter().enumerate() {
        assert_eq!(exact.memory.read_long_byte(CALLER + index as u32), 0);
        exact.memory.write_long_byte(CALLER + index as u32, byte);
    }
    for (x, z) in [
        (0, 0),
        (1, 0),
        (-1, 0),
        (5119, 0),
        (5120, 0),
        (5121, 0),
        (500, 500),
        (-500, 500),
        (500, -500),
        (-500, -500),
        (i16::MIN, 0),
        (i16::MIN, i16::MAX),
        (26790, 26790),
        (26791, 26791),
    ] {
        for yaw in u8::MIN..=u8::MAX {
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
                for (field, component) in POSITION
                    .into_iter()
                    .zip([position.x, position.y, position.z])
                {
                    exact.memory.write_word(base + field, component as u16);
                }
            }
            exact.memory.write_byte(VIEW + 21, yaw);
            exact.memory.write_word(0x1C31, 171);
            exact.memory.write_word(0x1C35, 0);
            exact.memory.write_word(0x1C37, 5120);
            exact.memory.write_word(0x1D16, 0);
            exact.run_retail_oracle_routine(CALLER, actor).unwrap();
            let spatial = OpeningFormationAudio { source }.spatial(listener);
            assert_eq!(
                exact.memory.read_word(0x1D16),
                if spatial.is_some() { 2 } else { 0 },
                "count x={x} z={z} yaw={yaw}"
            );
            if let Some(stereo) = spatial {
                assert_eq!(
                    exact.memory.read_word(0x1CF6),
                    171 + encoded_pan(stereo),
                    "payload x={x} z={z} yaw={yaw}"
                );
            }
        }
    }
}
