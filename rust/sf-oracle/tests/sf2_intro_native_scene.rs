//! Native opening actor integration. One test supplies observed CPU/GSU pass
//! partitioning as scheduler input, but never supplies actor poses, allocations
//! or RNG corrections. The separate ignored autonomous gate remains failing
//! until the timer/PPU refresh timing can be derived natively.
//! Artwork is also checked with native frame-barrier scheduling; that test
//! supplies only the initial decoded request, never publication boundaries or
//! palette corrections. A separate check retains individual source services.

use sf2_data::opening_artwork::{ForegroundPaletteId, OpeningArtwork};
use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_controller::{IntroColor, OpeningScenePalette, INTRO_PALETTE_COLORS};
use sf2_game::intro_scene::{OpeningAnimationFrame, OpeningScene, OpeningSceneActor};
use sf2_game::object::{
    object_address, object_index, ACTIVE_LIST, FIELD_PATH, FIELD_SHAPE, PLAYER_ONE,
};
use sf2_game::scene_artwork::{ArtworkPublication, ArtworkResume, ForegroundSelection};
use sf2_game::{RandomState, Vector3};
use sf_oracle::RetailMachine;

const WRAM: u32 = 0x7E0000;
const CONTROLLER: u32 = 0x0DBCCF;
const CAMERA_VIEW: u16 = 0x033F;
const UPDATE: u32 = 0x7F34E7;
const FIRST_VISIT: u32 = 0x7F3519;
const RESUME_VISIT: u32 = 0x7F3565;
const ENTROPY_REFRESH: u32 = 0x7F058F;

fn word(machine: &RetailMachine, address: u16) -> u16 {
    machine.peek16(WRAM + u32::from(address))
}

fn active_slots(machine: &RetailMachine) -> Vec<usize> {
    let mut slots = Vec::new();
    let mut cursor = word(machine, ACTIVE_LIST);
    while cursor != 0 {
        let slot = object_index(cursor).expect("retail active list contains a non-pool address");
        assert!(!slots.contains(&slot), "retail active-list cycle");
        slots.push(slot);
        cursor = word(machine, cursor);
    }
    slots
}

#[test]
fn native_actor_integration_with_observed_source_pass_partition() {
    check_opening_with_observed_source_pass_partition(false, false);
}

#[test]
fn opening_view_initialization_matches_native_default() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    let mut machine = RetailMachine::new(rom);
    machine.watch_cpu_execution(&[CONTROLLER]);
    assert!(machine
        .tick_until_cpu_execution(0, CONTROLLER, 240)
        .unwrap());
    let native = OpeningScene::default();
    let view = native.camera();
    assert_eq!(
        [0xC, 0xE, 0x10, 0x12, 0x14, 0x16, 0x29].map(|field| word(&machine, CAMERA_VIEW + field)),
        [
            view.position.x as u16,
            view.position.y as u16,
            view.position.z as u16,
            view.angles.pitch,
            view.angles.yaw,
            view.angles.roll,
            0
        ]
    );
    assert_eq!(native.render_view().position, view.position);
}

#[test]
fn native_palette_integration_with_observed_source_pass_partition() {
    check_opening_with_observed_source_pass_partition(true, false);
}

#[test]
fn native_palette_and_actor_integration_with_observed_source_services() {
    check_opening_with_observed_source_pass_partition(true, true);
}

fn check_opening_with_observed_source_pass_partition(check_palette: bool, observe_artwork: bool) {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("native opening verification requires the user-owned retail SF2 ROM");
    let artwork = std::sync::Arc::new(
        OpeningArtwork::decode(&rom[..0xB3FB8], &rom[..0xB44E4], &rom[..0xC2F24]).unwrap(),
    );
    let mut machine = RetailMachine::new(rom);
    // Next-node markers distinguish consecutive visits even if a strategy
    // returns without calling any other watched routine.
    machine.watch_cpu_execution(&[
        CONTROLLER,
        UPDATE,
        FIRST_VISIT,
        RESUME_VISIT,
        0x7F3531,
        0x7F357D,
        ENTROPY_REFRESH,
        0x03C80B, // Begin standard scene artwork.
        0x7F0BBF, // Setup's polygon palette copied.
        0x03C813, // Main loader resumes after setup.
        0x7F0CB2, // Character publication completed.
        0x7F0D08, // Map publication completed.
        0x03C893, // Main loader selected the standard foreground row.
        0x03C879,
        0x03C85F, // Alternate rows must not be silently treated as standard.
        0x03D509, // Foreground publication completed.
        0x03D520, // Main loader requests sprites.
        0x03D52C, // Sprite publication completed.
    ]);
    assert!(machine
        .tick_until_cpu_execution(0, CONTROLLER, 240)
        .unwrap());
    let player = word(&machine, PLAYER_ONE);
    let auxiliary = word(&machine, player + FIELD_PATH);
    assert_eq!(word(&machine, auxiliary.wrapping_add(0x6C16)), 0);
    assert_eq!(active_slots(&machine), vec![0, 2, 1]);
    assert_eq!(machine.peek8(WRAM + 0xC4), 1);
    let random = RandomState::new(std::array::from_fn(|i| {
        machine.peek8(WRAM + 0xE0 + i as u32)
    }));
    let colors = std::array::from_fn(|i| {
        IntroColor::from_bgr555(machine.peek16(WRAM + 0xEFE5 + i as u32 * 2))
    });
    let mut native = OpeningScene::new(random, OpeningScenePalette::new(colors));
    if check_palette && !observe_artwork {
        native.queue_artwork_load(artwork.clone(), false).unwrap();
    }
    machine.take_cpu_execution_watch_hits();
    let mut observed_splits = std::collections::BTreeSet::new();
    let mut draw_failures = std::collections::BTreeMap::new();
    let mut draw_checks = 0;
    for completed_updates in 1..=440 {
        assert!(machine
            .tick_until_cpu_execution(0, CONTROLLER, 240)
            .unwrap());
        let dispatches = machine.take_cpu_execution_watch_hits();
        let mut visits = 0;
        let mut budget = Vec::new();
        for &pc in &dispatches {
            match pc {
                0x7F3531 | 0x7F357D => visits += 1,
                ENTROPY_REFRESH => budget.push(visits),
                _ => {}
            }
        }
        observed_splits.insert(budget.clone());
        let native_events = native
            .tick_with_refresh_boundaries(&budget)
            .expect("native shared pool exhaustion");
        if check_palette && !observe_artwork {
            // These source events are assertions only. The native owner got
            // no service-boundary input: it retained the frame-buffer roles,
            // joined this frame and applied its pending artwork request.
            let expected_publications: Vec<_> = dispatches
                .iter()
                .filter_map(|pc| match pc {
                    0x7F0BBF => Some(ArtworkPublication::PolygonPalette),
                    0x7F0CB2 => Some(ArtworkPublication::BackgroundCharacters),
                    0x7F0D08 => Some(ArtworkPublication::BackgroundMap),
                    0x03D509 => Some(ArtworkPublication::ForegroundPalette(
                        ForegroundPaletteId::Standard,
                    )),
                    0x03D52C => Some(ArtworkPublication::SpritePalette),
                    _ => None,
                })
                .collect();
            assert_eq!(
                native_events.artwork_publications, expected_publications,
                "artwork publications update={completed_updates}"
            );
        }
        if observe_artwork {
            // The controller is the first actor and the only actor writing
            // this palette. These events follow its just-completed visit and
            // precede the next controller entry. Do not inject original colors
            // or replace this event order with a completed-update number.
            for &pc in &dispatches {
                let publication = match pc {
                    0x03C80B => {
                        // The boot request does not inherit a palette-skip flag.
                        assert_eq!(machine.peek16(WRAM + 0x1B9C) & 0x0040, 0);
                        native.begin_artwork_load(artwork.clone(), false).unwrap();
                        None
                    }
                    0x7F0BBF => Some(ArtworkPublication::PolygonPalette),
                    0x03C813 => {
                        assert_eq!(
                            native
                                .resume_artwork_load(ForegroundSelection::STANDARD)
                                .unwrap(),
                            ArtworkResume::Queued(ArtworkPublication::BackgroundCharacters)
                        );
                        None
                    }
                    0x7F0CB2 => Some(ArtworkPublication::BackgroundCharacters),
                    0x7F0D08 => Some(ArtworkPublication::BackgroundMap),
                    0x03C893 => {
                        assert_eq!(
                            native
                                .resume_artwork_load(ForegroundSelection::STANDARD)
                                .unwrap(),
                            ArtworkResume::Queued(ArtworkPublication::ForegroundPalette(
                                ForegroundPaletteId::Standard
                            ))
                        );
                        None
                    }
                    0x03C879 | 0x03C85F => {
                        panic!("unexpected alternate foreground in neutral boot")
                    }
                    0x03D509 => Some(ArtworkPublication::ForegroundPalette(
                        ForegroundPaletteId::Standard,
                    )),
                    0x03D520 => {
                        assert_eq!(
                            native
                                .resume_artwork_load(ForegroundSelection::STANDARD)
                                .unwrap(),
                            ArtworkResume::Queued(ArtworkPublication::SpritePalette)
                        );
                        None
                    }
                    0x03D52C => Some(ArtworkPublication::SpritePalette),
                    _ => None,
                };
                if let Some(publication) = publication {
                    assert_eq!(native.publish_artwork(), Some(publication));
                }
            }
        }
        assert_eq!(
            word(&machine, auxiliary.wrapping_add(0x6C16)),
            completed_updates
        );
        assert_eq!(native.controller().elapsed_updates(), completed_updates);
        if check_palette {
            // Compare state, never inject source colors after initialization.
            for (index, (live, saved)) in native
                .palette()
                .colors
                .iter()
                .zip(&native.palette().saved_colors)
                .enumerate()
            {
                assert_eq!(
                    machine.peek16(WRAM + 0xEFE5 + index as u32 * 2),
                    live.bgr555(),
                    "live palette update={completed_updates} color={index}"
                );
                assert_eq!(
                    machine.peek16(WRAM + 0xF2E5 + index as u32 * 2),
                    saved.bgr555(),
                    "saved palette update={completed_updates} color={index}"
                );
            }
        }
        let cue = match native.controller().cue() {
            OpeningCameraCue::Opening => 1,
            OpeningCameraCue::FirstCut => 2,
            OpeningCameraCue::SecondCut => 3,
            OpeningCameraCue::ThirdCut => 4,
            OpeningCameraCue::FourthCut => 5,
            OpeningCameraCue::FinalCut => 6,
        };
        assert_eq!(
            machine.peek8(WRAM + 0x1D72),
            cue,
            "cue update={completed_updates}"
        );
        assert_eq!(
            machine.peek8(WRAM + 0xC4),
            native.global_clock().wrapping_add(1)
        );
        let slots: Vec<_> = native.actors().map(|(id, _)| id.index()).collect();
        assert_eq!(
            active_slots(&machine),
            slots,
            "ordered slots update={completed_updates} budget={budget:?}"
        );
        for (id, actor) in native.actors() {
            if matches!(
                actor,
                OpeningSceneActor::Controller | OpeningSceneActor::InactivePlayer
            ) {
                continue;
            }
            let source = object_address(id.index());
            if actor.shape() != sf2_game::ShapeId::EMPTY {
                let controls = actor.draw_controls();
                let source_sort_bias = if machine.peek8(WRAM + u32::from(source + 9)) & 1 != 0 {
                    15_000
                } else {
                    0
                };
                let expected = [
                    source_sort_bias as u16,
                    u16::from(machine.peek8(WRAM + u32::from(source + 0x1CC8))),
                    word(&machine, source + 0x1CCD),
                    u16::from(machine.peek8(WRAM + u32::from(source + 0x1CDB))),
                    u16::from(machine.peek8(WRAM + u32::from(source + 0x23)) & 2 == 0),
                    u16::from(machine.peek8(WRAM + u32::from(source + 0x1CCB))),
                    u16::from(machine.peek8(WRAM + u32::from(source + 0x1CCA))),
                ];
                let frame_control = |frame| match frame {
                    OpeningAnimationFrame::SceneClock => 0,
                    OpeningAnimationFrame::Authored(frame) => u16::from(frame | 128),
                };
                let actual = [
                    controls.sort_bias as u16,
                    u16::from(controls.depth_offset),
                    controls
                        .material_override
                        .map_or(0, |material| material.catalog_token()),
                    u16::from(controls.texture_scroll_y),
                    u16::from(actor.is_visible()),
                    frame_control(controls.shape_frame),
                    frame_control(controls.color_frame),
                ];
                for (field, (expected, actual)) in [
                    "sort bias",
                    "depth offset",
                    "material",
                    "texture scroll",
                    "visibility",
                    "shape frame control",
                    "color frame control",
                ]
                .into_iter()
                .zip(expected.into_iter().zip(actual))
                {
                    if expected != actual {
                        draw_failures.entry((id.index(), field)).or_insert_with(||
                            format!("update={completed_updates} expected={expected} actual={actual} actor={actor:?}"));
                    }
                }
                draw_checks += 1;
            }
            assert_eq!(
                word(&machine, source + FIELD_SHAPE),
                0xBC9C_u16.wrapping_add(actor.shape().catalog_index() as u16 * 28),
                "shape update={completed_updates} slot={} budget={budget:?} actor={actor:?}",
                id.index()
            );
            let [x, y, z] = [12, 14, 16].map(|field| word(&machine, source + field) as i16);
            assert_eq!(
                Vector3 { x, y, z },
                actor.pose().position,
                "position update={completed_updates} slot={} budget={budget:?} actor={actor:?}",
                id.index()
            );
            // Common explosions are billboard actors whose retained source
            // angle bytes are not represented by IntroExplosionActor.
            if !matches!(
                actor,
                OpeningSceneActor::Explosion(_)
                    | OpeningSceneActor::SecondFlyby(
                        sf2_game::intro_second_flyby_scene::OpeningSecondFlybyActor::Explosion(_)
                    )
            ) {
                let rotation = actor.pose().rotation;
                for (field, angle) in
                    [18, 20, 22]
                        .into_iter()
                        .zip([rotation.pitch, rotation.yaw, rotation.roll])
                {
                    assert_eq!(
                        machine.peek8(WRAM + u32::from(source + field)),
                        angle.units(),
                        "rotation update={completed_updates} slot={} field={field} actor={actor:?}",
                        id.index()
                    );
                }
            }
        }
        let random: [u8; 4] = std::array::from_fn(|i| machine.peek8(WRAM + 0xE0 + i as u32));
        assert_eq!(
            random,
            native.random().bytes(),
            "RNG update={completed_updates} budget={budget:?}"
        );
        {
            assert_eq!(
                word(&machine, CAMERA_VIEW + 0x29),
                0,
                "camera follow distance after {completed_updates} updates"
            );
            let view = native.camera();
            assert_eq!(
                native.render_view().position,
                view.position,
                "opening render anchor after {completed_updates} updates"
            );
            assert_eq!(
                native.render_view().matrix,
                sf_core::snes_trig::zxy_matrix_q15_fine(
                    word(&machine, CAMERA_VIEW + 0x12),
                    word(&machine, CAMERA_VIEW + 0x14),
                    word(&machine, CAMERA_VIEW + 0x16),
                ),
                "opening render matrix after {completed_updates} updates"
            );
            let [x, y, z] = [12, 14, 16].map(|field| word(&machine, CAMERA_VIEW + field) as i16);
            assert_eq!(
                Vector3 { x, y, z },
                view.position,
                "camera update={completed_updates}"
            );
            for (field, angle) in
                [18, 20, 22]
                    .into_iter()
                    .zip([view.angles.pitch, view.angles.yaw, view.angles.roll])
            {
                assert_eq!(
                    word(&machine, CAMERA_VIEW + field),
                    angle,
                    "camera angle update={completed_updates} field={field}"
                );
            }
        }
    }
    assert!(
        observed_splits.len() > 1,
        "boot must exercise variable pass partitioning"
    );
    assert!(
        draw_checks > 1000,
        "opening must exercise complete actor lifecycles"
    );
    assert!(
        draw_failures.is_empty(),
        "opening draw controls: {draw_failures:#?}"
    );
}

#[test]
#[ignore = "known failure at update 101: native timer/PPU refresh timing is not implemented"]
fn native_opening_matches_boot_actor_pool_through_last_opening_boundary() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("native opening verification requires the user-owned retail SF2 ROM");
    let mut machine = RetailMachine::new(rom);
    machine.watch_cpu_execution(&[CONTROLLER, 0x7F7BD4, 0x7F34E7, 0x7F354A, 0x7F402D]);
    assert!(machine
        .tick_until_cpu_execution(0, CONTROLLER, 240)
        .unwrap());
    let player = word(&machine, PLAYER_ONE);
    let auxiliary = word(&machine, player + FIELD_PATH);
    assert_eq!(word(&machine, auxiliary.wrapping_add(0x6C16)), 0);
    let random = RandomState::new(std::array::from_fn(|i| {
        machine.peek8(WRAM + 0xE0 + i as u32)
    }));
    let colors: [IntroColor; INTRO_PALETTE_COLORS] = std::array::from_fn(|i| {
        IntroColor::from_bgr555(machine.peek16(WRAM + 0xEFE5 + i as u32 * 2))
    });
    let mut native = OpeningScene::new(random, OpeningScenePalette::new(colors));
    assert_eq!(active_slots(&machine), vec![0, 2, 1]);
    machine.take_cpu_execution_watch_hits();

    // At entry N, the previous traversal (N-1) is complete and the source
    // has incremented its global clock for traversal N. Native tick advances
    // exactly one actor traversal, independently of video-frame duration.
    for completed_updates in 1..=440 {
        let random_before = native.random().bytes();
        native
            .tick()
            .expect("native opening exhausted its shared pool");
        machine.arm_wram_write_watch(0xE0);
        assert!(machine
            .tick_until_cpu_execution(0, CONTROLLER, 240)
            .unwrap());
        let dispatches = machine.take_cpu_execution_watch_hits();
        let random_writes = machine.take_wram_write_watch();
        assert_eq!(
            word(&machine, auxiliary.wrapping_add(0x6C16)),
            completed_updates,
            "controller boundary"
        );
        assert_eq!(
            machine.peek8(WRAM + 0xC4),
            native.global_clock().wrapping_add(1),
            "global actor clock at next controller entry"
        );
        let native_slots: Vec<_> = native.actors().map(|(id, _)| id.index()).collect();
        assert_eq!(
            active_slots(&machine),
            native_slots,
            "ordered actor slots after {completed_updates} updates"
        );
        for (id, actor) in native.actors() {
            // Player records are scheduling/view state, not reconstructed
            // geometry; both still participate in the ordered-list assertion.
            if matches!(
                actor,
                OpeningSceneActor::Controller | OpeningSceneActor::InactivePlayer
            ) {
                continue;
            }
            let source = object_address(id.index());
            assert_eq!(
                word(&machine, source + FIELD_SHAPE),
                0xBC9C_u16.wrapping_add(actor.shape().catalog_index() as u16 * 28),
                "shape after {completed_updates} updates slot={} actor={actor:?}",
                id.index()
            );
            let [x, y, z] = [12, 14, 16].map(|field| word(&machine, source + field) as i16);
            assert_eq!(
                Vector3 { x, y, z },
                actor.pose().position,
                "position after {completed_updates} updates slot={} actor={actor:?} rng_before={random_before:?} rng_native={:?} rng_retail={:?} dispatches={dispatches:06X?} random_writes={random_writes:06X?}",
                id.index(), native.random().bytes(),
                [0xE0, 0xE1, 0xE2, 0xE3].map(|address| machine.peek8(WRAM + address))
            );
        }
        let retail_random: [u8; 4] = std::array::from_fn(|i| machine.peek8(WRAM + 0xE0 + i as u32));
        assert_eq!(
            retail_random,
            native.random().bytes(),
            "shared RNG after {completed_updates} updates dispatches={dispatches:06X?}"
        );
        let cue = match native.controller().cue() {
            OpeningCameraCue::Opening => 1,
            OpeningCameraCue::FirstCut => 2,
            OpeningCameraCue::SecondCut => 3,
            OpeningCameraCue::ThirdCut => 4,
            OpeningCameraCue::FourthCut => 5,
            OpeningCameraCue::FinalCut => 6,
        };
        assert_eq!(machine.peek8(WRAM + 0x1D72), cue, "camera cue");
        if completed_updates >= 2 {
            let view = native.camera();
            // Camera publication uses the reserved view object, distinct
            // from both pooled players in a real boot.
            let [x, y, z] = [12, 14, 16].map(|field| word(&machine, CAMERA_VIEW + field) as i16);
            assert_eq!(
                Vector3 { x, y, z },
                view.position,
                "camera position after {completed_updates} updates"
            );
            for (field, angle) in
                [18, 20, 22]
                    .into_iter()
                    .zip([view.angles.pitch, view.angles.yaw, view.angles.roll])
            {
                assert_eq!(
                    word(&machine, CAMERA_VIEW + field),
                    angle,
                    "camera angle field={field} after {completed_updates} updates"
                );
            }
        }
    }
    // Intentionally stop before the final opening call: this checks shared
    // actor integration, not scene handoff, audio output, or rendered pixels.
}
