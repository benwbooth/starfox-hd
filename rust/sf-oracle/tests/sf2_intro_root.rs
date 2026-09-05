//! Execute the unmodified opening root and its allocation, movement and
//! attachment helpers. Child strategies are intentionally not executed here;
//! this isolates root-authored spawn events from their consumers.

use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_root::{
    OpeningAttachmentGroup, OpeningFormationMember, OpeningRootActor, OpeningRootEvent,
    OpeningRootPhase, OpeningRootSpawn, OpeningSceneRoot, OpeningSpawnPlacement,
};
use sf2_game::object::{
    active_objects, allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY, PLAYER_ONE,
    SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{Rotation, Vector3};

const ROOT_PATH: u16 = 0xFA11;
const INITIAL_STRATEGY: u32 = 0x7F7E1E;
const VIEW: u16 = 0x033F;
const AUX: u16 = 0x0140;
const POSITION: [u16; 3] = [0x0C, 0x0E, 0x10];
const ROTATION: [u16; 3] = [0x12, 0x14, 0x16];
const LOCAL_POSITION: [u16; 3] = [0x1CCF, 0x1CD1, 0x1CD3];
const LOCAL_ROTATION: [u16; 3] = [0x1CD5, 0x1CD6, 0x1CD7];
const CUE: u16 = 0x1D72;
const PARENT: u16 = 6;
const CHILD_GROUP: u16 = 0x13;
const FLAGS: u16 = 0x25;
const REMOVE_BIT: u8 = 8;
const SHAPE_BASE: u16 = 0xBC9C;
const SHAPE_STRIDE: u16 = 28;
const AUDIO_QUEUE: u16 = 0x1CF6;
const AUDIO_TAIL: u16 = 0x1D16;
const FLYBY_MARKER: u16 = 0xBD;
const BACKGROUND_HORIZONTAL: u16 = 0x1E4E;
const BACKGROUND_VERTICAL: u16 = 0x1E44;
const DEPTH_VELOCITY: u16 = 0x1E20;
const SAVED_POSITION: u16 = 0x6BED + AUX;
const SAVED_ROTATION: u16 = 0x6AB9 + AUX;
const ANCHOR_SENTINEL: u16 = 0xA55A;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("root differential tests require the user-owned retail SF2 ROM")
}

fn assert_position(exact: &Game, actor: u16, fields: [u16; 3], expected: Vector3, update: usize) {
    for (field, value) in fields.into_iter().zip([expected.x, expected.y, expected.z]) {
        assert_eq!(
            exact.memory.read_word(actor + field) as i16,
            value,
            "actor {actor}, field {field}, update {update}"
        );
    }
}

fn assert_rotation(exact: &Game, actor: u16, fields: [u16; 3], expected: Rotation, update: usize) {
    for (field, value) in fields
        .into_iter()
        .zip([expected.pitch, expected.yaw, expected.roll])
    {
        assert_eq!(
            exact.memory.read_byte(actor + field),
            value.units(),
            "actor {actor}, field {field}, update {update}"
        );
    }
}

fn spawn_encoding(actor: OpeningRootActor) -> (u16, u8, u8) {
    match actor {
        OpeningRootActor::CameraTarget => (0xFAB2, 1, 1),
        OpeningRootActor::NintendoLogo => (0x9284, 1, 1),
        OpeningRootActor::Camera => (0xFB4C, 1, 0),
        OpeningRootActor::FlybyRig => (0xFD5C, 1, 1),
        OpeningRootActor::AttachedCraft => (0xFCF2, 1, 1),
        OpeningRootActor::FreeCraft => (0xFCC5, 1, 5),
        OpeningRootActor::FormationCraft(OpeningFormationMember::First) => (0xFBD0, 18, 0),
        OpeningRootActor::FormationCraft(OpeningFormationMember::Second) => (0xFBD0, 15, 1),
        OpeningRootActor::FormationCraft(OpeningFormationMember::Third) => (0xFBD0, 20, 2),
        OpeningRootActor::SecondFlybyCraft => (0xFDC2, 1, 16),
        OpeningRootActor::SecondCameraTarget => (0xFB08, 1, 15),
    }
}

fn group_encoding(group: OpeningAttachmentGroup) -> u8 {
    match group {
        OpeningAttachmentGroup::TrackingAndCraft => 1,
        OpeningAttachmentGroup::FlybyRig => 3,
    }
}

fn cue_encoding(cue: OpeningCameraCue) -> u8 {
    match cue {
        OpeningCameraCue::Opening => 1,
        OpeningCameraCue::FirstCut => 2,
        OpeningCameraCue::SecondCut => 3,
        OpeningCameraCue::ThirdCut => 4,
        OpeningCameraCue::FourthCut => 5,
        OpeningCameraCue::FinalCut => 6,
    }
}

fn schedule(case: usize, update: usize) -> OpeningCameraCue {
    use OpeningCameraCue::*;
    match case {
        0 => match update {
            0..182 => Opening,
            182..249 => FirstCut,
            249..293 => SecondCut,
            293..327 => ThirdCut,
            327..416 => FourthCut,
            _ => FinalCut,
        },
        // The first cut arrives before the 96-update wait ends, but persists.
        1 => match update {
            0 => Opening,
            1..98 => FirstCut,
            98..100 => ThirdCut,
            _ => FourthCut,
        },
        // Missing equality gates must not advance the root.
        2 => FinalCut,
        3 => match update {
            0..110 => Opening,
            110..112 => FirstCut,
            _ => FourthCut,
        },
        // All later gates remain in one cue: no inferred end-of-scene timeout.
        _ => Opening,
    }
}

#[test]
fn complete_root_matches_retail_spawns_waits_cue_gates_and_persistent_motion() {
    let rom = retail();
    for case in 0..5 {
        let mut exact = Game::new(rom.clone()).unwrap();
        let root = allocate(&mut exact.memory, 0).unwrap();
        exact.memory.write_word(root + FIELD_PATH, ROOT_PATH);
        exact
            .memory
            .write_word(root + FIELD_STRATEGY, INITIAL_STRATEGY as u16);
        exact
            .memory
            .write_byte(root + FIELD_STRATEGY + 2, (INITIAL_STRATEGY >> 16) as u8);
        exact.memory.write_byte(root + 0x2D, 1);
        exact.memory.write_word(root + FIELD_SHAPE, SHAPE_BASE);
        exact.memory.write_word(PLAYER_ONE, VIEW);
        exact.memory.write_word(SELECTED_OBJECT, VIEW);
        exact.memory.write_word(VIEW + FIELD_PATH, AUX);
        for field in POSITION.into_iter().chain(ROTATION) {
            exact.memory.write_word(VIEW + field, ANCHOR_SENTINEL);
        }
        for field in [0, 2, 4] {
            exact
                .memory
                .write_word(SAVED_POSITION + field, ANCHOR_SENTINEL);
            exact
                .memory
                .write_word(SAVED_ROTATION + field, ANCHOR_SENTINEL);
        }
        let mut native = OpeningSceneRoot::default();
        let mut spawned: Vec<(u16, OpeningRootSpawn)> = Vec::new();
        let mut removed = Vec::new();
        let max_updates = if case == 0 { 4_200 } else { 500 };
        for update in 0..max_updates {
            let cue = schedule(case, update);
            exact.memory.write_byte(CUE, cue_encoding(cue));
            // The root does not opt into ambient horizontal/depth scrolling.
            exact
                .memory
                .write_word(0x1E1C, (update as i16).wrapping_mul(17) as u16);
            exact
                .memory
                .write_word(DEPTH_VELOCITY, (update as i16).wrapping_mul(-11) as u16);
            exact.memory.write_word(CURRENT_OBJECT, root);
            let strategy = u32::from(exact.memory.read_word(root + FIELD_STRATEGY))
                | (u32::from(exact.memory.read_byte(root + FIELD_STRATEGY + 2)) << 16);
            let previous_audio_tail = exact.memory.read_word(AUDIO_TAIL);
            exact.run_retail_oracle_routine(strategy, root).unwrap();
            let events = native.tick(cue);
            let mut allocations: Vec<_> = active_objects(&exact.memory)
                .into_iter()
                .filter(|actor| *actor != root && !spawned.iter().any(|(old, _)| old == actor))
                .collect();
            allocations.sort_unstable();
            let mut allocations = allocations.into_iter();
            let mut queued_audio = false;
            for event in events {
                match event {
                    OpeningRootEvent::Initialize {
                        background_origin,
                        player_anchor,
                        depth_velocity,
                        cue,
                    } => {
                        assert_eq!(update, 0);
                        assert_eq!(
                            exact.memory.read_word(BACKGROUND_HORIZONTAL) as i16,
                            background_origin.horizontal
                        );
                        assert_eq!(
                            exact.memory.read_word(BACKGROUND_VERTICAL) as i16,
                            background_origin.vertical
                        );
                        assert_eq!(
                            exact.memory.read_word(DEPTH_VELOCITY) as i16,
                            depth_velocity
                        );
                        assert_eq!(exact.memory.read_byte(CUE), cue_encoding(cue));
                        assert_position(
                            &exact,
                            SAVED_POSITION,
                            [0, 2, 4],
                            player_anchor.retained_position,
                            update,
                        );
                        assert_position(&exact, VIEW, POSITION, player_anchor.position, update);
                        assert_rotation(&exact, VIEW, ROTATION, player_anchor.rotation, update);
                        assert_eq!(
                            exact.memory.read_word(SAVED_ROTATION),
                            player_anchor.retained_rotation.pitch
                        );
                        assert_eq!(
                            exact.memory.read_word(SAVED_ROTATION + 2),
                            player_anchor.retained_rotation.yaw
                        );
                        assert_eq!(
                            exact.memory.read_byte(SAVED_ROTATION + 4),
                            player_anchor.retained_rotation.roll.units()
                        );
                    }
                    OpeningRootEvent::Spawn(spawn) => {
                        let actor = allocations
                            .next()
                            .expect("native emitted a spawn absent from retail");
                        let (path, parameter, variant) = spawn_encoding(spawn.actor);
                        assert_eq!(
                            exact.memory.read_word(actor + FIELD_PATH),
                            path,
                            "update {update}"
                        );
                        assert_eq!(
                            exact.memory.read_word(actor + FIELD_SHAPE),
                            SHAPE_BASE + spawn.actor.shape().catalog_index() as u16 * SHAPE_STRIDE
                        );
                        assert_eq!(exact.memory.read_byte(actor + 0x2D), parameter);
                        assert_eq!(exact.memory.read_byte(actor + 0x2E), variant);
                        match spawn.placement {
                            OpeningSpawnPlacement::Independent(pose) => {
                                assert_position(&exact, actor, POSITION, pose.position, update);
                                assert_rotation(&exact, actor, ROTATION, pose.rotation, update);
                                assert_eq!(exact.memory.read_byte(actor + 0x23) & 4, 0);
                            }
                            OpeningSpawnPlacement::Attached { local, group } => {
                                assert_eq!(exact.memory.read_word(actor + PARENT), root);
                                assert_eq!(
                                    exact.memory.read_byte(actor + CHILD_GROUP),
                                    group_encoding(group)
                                );
                                assert_position(
                                    &exact,
                                    actor,
                                    LOCAL_POSITION,
                                    local.offset,
                                    update,
                                );
                                assert_rotation(
                                    &exact,
                                    actor,
                                    LOCAL_ROTATION,
                                    local.rotation,
                                    update,
                                );
                            }
                        }
                        spawned.push((actor, spawn));
                    }
                    OpeningRootEvent::QueueFlybyAudio => {
                        assert!(!queued_audio);
                        queued_audio = true;
                        assert_eq!(
                            exact.memory.read_word(AUDIO_QUEUE + previous_audio_tail),
                            FLYBY_MARKER
                        );
                    }
                    OpeningRootEvent::RemoveFirstAttachment(group) => {
                        let &(actor, _) = spawned.iter().find(|(_, spawn)| matches!(spawn.placement,
                            OpeningSpawnPlacement::Attached { group: candidate, .. } if candidate == group)).unwrap();
                        removed.push(actor);
                    }
                }
            }
            assert!(
                allocations.next().is_none(),
                "unmodeled retail spawn, case {case}, update {update}"
            );
            assert_eq!(
                exact.memory.read_word(AUDIO_TAIL),
                previous_audio_tail + if queued_audio { 2 } else { 0 }
            );
            assert_position(&exact, root, POSITION, native.pose.position, update);
            assert_rotation(&exact, root, ROTATION, native.pose.rotation, update);
            assert_position(
                &exact,
                root,
                [0x32, 0x34, 0x36],
                Vector3 {
                    x: 0,
                    y: 0,
                    z: native.depth_velocity(),
                },
                update,
            );
            for &(actor, spawn) in &spawned {
                assert_eq!(
                    exact.memory.read_byte(actor + FLAGS) & REMOVE_BIT != 0,
                    removed.contains(&actor)
                );
                if let OpeningSpawnPlacement::Attached { local, .. } = spawn.placement {
                    let pose = local.world_pose(native.pose);
                    assert_position(&exact, actor, POSITION, pose.position, update);
                    assert_rotation(&exact, actor, ROTATION, pose.rotation, update);
                }
            }
            let path = match native.phase() {
                OpeningRootPhase::Initializing => unreachable!(),
                OpeningRootPhase::WaitingForFlyby { .. } => 0xFA59,
                OpeningRootPhase::AwaitingFirstCut => 0xFA88,
                OpeningRootPhase::FirstCutPause => 0xFA91,
                OpeningRootPhase::AwaitingThirdCut => 0xFA9A,
                OpeningRootPhase::AwaitingFourthCut => 0xFAA9,
                OpeningRootPhase::Holding => 0xFAB1,
            };
            assert_eq!(
                exact.memory.read_word(root + FIELD_PATH),
                path,
                "case {case}, update {update}"
            );
        }
        assert_eq!(
            spawned.len(),
            match case {
                0 | 1 => 11,
                3 => 10,
                _ => 9,
            }
        );
        if case == 0 {
            assert!(
                native.pose.position.z < 0,
                "the long run must cover coordinate wrapping"
            );
        }
    }
}
