//! Strict composed-video anchors around the source-hardware-only launch
//! aperture cadence interval.

#[path = "../examples/support/sf1_mesen_video.rs"]
mod mesen_video;
#[path = "../examples/support/mod.rs"]
mod support;
#[path = "../examples/support/sf1_timing.rs"]
mod timing_entry;
#[path = "../examples/support/sf1_video.rs"]
mod video;

use sf_core::stage_banner::ScrambleBannerState;
use sf_difftest::{
    compare_source_rgb, write_source_rgb_ppm, SOURCE_FRAME_HEIGHT, SOURCE_FRAME_WIDTH,
};
use sf_game::shell::FrameSnapshot;
use sf_oracle::{
    load_retail_rom, RetailMachine, RETAIL_BUILD_DRAWLIST_L, RETAIL_DOSTRATS, RETAIL_GAMEFRAME,
    RETAIL_SCRAMBLE_COUNT,
};
use sf_render::{
    draw_list::DrawListEntry,
    renderer::{config_from_repo_root, FrameInputs, GameState as RenderGameState, Renderer},
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, VecDeque};

const RETAIL_ROM_SHA256: &str = "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548";
const WORK_RAM: u32 = 0x7E_0000;
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const FIRST_APERTURE_ANCHOR: u16 = 7;
const POST_APERTURE_ANCHOR: u16 = 20;
const WARNING_LAYER_ANCHOR: u16 = 21;
const COMPOSED_ANCHORS: [u16; 16] = [
    5,
    6,
    FIRST_APERTURE_ANCHOR,
    8,
    9,
    10,
    11,
    12,
    13,
    14,
    15,
    16,
    17,
    18,
    19,
    POST_APERTURE_ANCHOR,
];
const CAPTURE_ANCHORS: [u16; 17] = [
    5,
    6,
    FIRST_APERTURE_ANCHOR,
    8,
    9,
    10,
    11,
    12,
    13,
    14,
    15,
    16,
    17,
    18,
    19,
    POST_APERTURE_ANCHOR,
    WARNING_LAYER_ANCHOR,
];
const WARNING_LEFT: usize = 72;
// The sprite formatter writes Y=72; its first visible row is hardware line 73.
const WARNING_TOP: usize = 73;
const WARNING_WIDTH: usize = 128;
const WARNING_HEIGHT: usize = 16;
const WARNING_OPAQUE_PIXELS: usize = 1_671;
const SCANOUT_DRAIN_UPDATES: u16 = 4;

#[test]
fn retail_launch_video_matches_before_and_after_variable_scanout_cadence() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("launch video anchors skipped: Star Fox retail ROM not found");
        return;
    };
    assert_eq!(
        format!("{:x}", Sha256::digest(&rom)),
        RETAIL_ROM_SHA256,
        "launch video anchors require the pinned Star Fox USA Rev 2 ROM"
    );

    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let mut renderer = Renderer::new_headless(
        SOURCE_FRAME_WIDTH as i32,
        SOURCE_FRAME_HEIGHT as i32,
        &config_from_repo_root(repository),
    )
    .expect("headless launch anchor renderer");
    let mut retail = RetailMachine::new(rom);
    let mut native = support::configured_shell();
    timing_entry::enter_first_corneria_update(&mut retail).expect("source-bound launch entry");
    timing_entry::enter_native_corneria_update(&mut native).expect("native launch entry");
    retail.capture_completed_rasters();
    let mut pending: Option<(u16, FrameSnapshot, Vec<DrawListEntry>, Vec<u8>)> = None;
    let mut pending_video = VecDeque::new();
    let mut completed_rasters = VecDeque::new();
    let mut certified = BTreeSet::new();
    let mut warning_layer_certified = false;
    let mut first_video_divergence = None;
    let mesen = std::env::var_os("SF1_LAUNCH_MESEN_DIR")
        .map(|directory| mesen_video::LaunchVideo::read(std::path::Path::new(&directory)));

    for tick in 0..u32::from(WARNING_LAYER_ANCHOR + SCANOUT_DRAIN_UPDATES) {
        assert!(
            retail
                .tick_until_cpu_execution(
                    0,
                    RETAIL_BUILD_DRAWLIST_L,
                    MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE
                )
                .expect("launch draw-list boundary"),
            "retail did not complete launch draw list at update {tick}"
        );
        let retail_scene_draws = support::retail_source_draws(&retail);
        let retail_scene_camera = [0x00C1, 0x00C3, 0x00C5, 0x1633, 0x1635, 0x1637]
            .map(|address| retail.peek16(WORK_RAM | address) as i16);
        assert!(
            retail
                .tick_until_cpu_execution(0, RETAIL_DOSTRATS, MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE)
                .expect("next launch gameplay boundary"),
            "retail did not reach the next launch update {tick}"
        );
        completed_rasters.extend(retail.take_completed_rasters());
        let retail_level_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        native.tick(0);
        let game_frame = native.game.vars.gameframe;
        if std::env::var_os("SF1_LAUNCH_DIAGNOSTIC").is_some() {
            eprintln!(
                "launch_hud scene={game_frame} original_count={} native_count={} wipe={:?} brightness={} black={} stayblack={}",
                retail.peek8(WORK_RAM | RETAIL_SCRAMBLE_COUNT),
                native.game.vars.scramble_count,
                native.frame().screen_wipe,
                native.frame().display_brightness,
                native.frame().display_black_subtraction,
                native.frame().stayblack
            );
        }
        assert_eq!(
            game_frame,
            tick as u16 + 1,
            "native skipped a launch update"
        );
        assert_eq!(game_frame, retail_level_frame, "launch strategy boundary");
        let current_draw_list = native
            .draw_list()
            .iter()
            .map(support::render_entry)
            .collect::<Vec<_>>();
        let camera = native.frame().camera;
        assert_eq!(
            retail_scene_camera,
            [
                (camera.x >> 16) as i16,
                (camera.y >> 16) as i16,
                (camera.z >> 16) as i16,
                camera.rotation[0] as i16,
                camera.rotation[1] as i16,
                camera.rotation[2] as i16,
            ],
            "launch camera at scene {game_frame}",
        );
        if game_frame == FIRST_APERTURE_ANCHOR {
            assert_eq!(
                native.game.vars.scramble_count, 50,
                "Corneria source map must arm the SCRAMBLE presentation"
            );
        }
        if CAPTURE_ANCHORS.contains(&game_frame) {
            assert_eq!(
                support::native_source_draws(&native),
                retail_scene_draws,
                "launch scene draws at game frame {game_frame}"
            );
        }

        if let Some((pending_game_frame, pending_frame, pending_draw_list, retail_bitmap)) =
            pending.take()
        {
            assert_eq!(game_frame, pending_game_frame + 1);
            if COMPOSED_ANCHORS.contains(&pending_game_frame) {
                let native_rgb = support::render_presentation_aligned_source_frame(
                    &pending_frame,
                    &native.frame(),
                    &pending_draw_list,
                    &mut renderer,
                );
                pending_video.push_back((pending_game_frame, retail_bitmap, native_rgb));
            } else {
                assert_eq!(pending_game_frame, WARNING_LAYER_ANCHOR);
                assert!(
                    retail.peek8(WORK_RAM | RETAIL_SCRAMBLE_COUNT) != 0,
                    "original warning countdown remains active"
                );
                let retail_objects = retail.ppu_snapshot_obj_rgba();

                let mut isolated = FrameInputs {
                    source_resolution: true,
                    game_state: RenderGameState::Playing,
                    scramble_banner: Some(ScrambleBannerState {
                        ticks_remaining: 50,
                        game_frame: pending_game_frame,
                    }),
                    ..FrameInputs::default()
                };
                renderer.begin_frame();
                renderer.submit(&[], &[], 0.0, &isolated);
                renderer.end_frame();
                let with_warning = renderer.read_pixels_rgb();

                isolated.scramble_banner = None;
                renderer.begin_frame();
                renderer.submit(&[], &[], 0.0, &isolated);
                renderer.end_frame();
                let without_warning = renderer.read_pixels_rgb();

                let mut changed_pixels = 0;
                for y in 0..SOURCE_FRAME_HEIGHT {
                    for x in 0..SOURCE_FRAME_WIDTH {
                        let rgb_offset = (y * SOURCE_FRAME_WIDTH + x) * 3;
                        let rgba_offset = (y * SOURCE_FRAME_WIDTH + x) * 4;
                        let changed = with_warning[rgb_offset..rgb_offset + 3]
                            != without_warning[rgb_offset..rgb_offset + 3];
                        let retail_opaque = retail_objects[rgba_offset + 3] != 0;
                        let in_warning = (WARNING_LEFT..WARNING_LEFT + WARNING_WIDTH).contains(&x)
                            && (WARNING_TOP..WARNING_TOP + WARNING_HEIGHT).contains(&y);
                        assert_eq!(
                            changed,
                            retail_opaque && in_warning,
                            "warning OBJ coverage at {x},{y}"
                        );
                        if changed {
                            changed_pixels += 1;
                            assert_eq!(
                                &with_warning[rgb_offset..rgb_offset + 3],
                                &retail_objects[rgba_offset..rgba_offset + 3],
                                "warning OBJ color at {x},{y}"
                            );
                        }
                    }
                }
                assert_eq!(changed_pixels, WARNING_OPAQUE_PIXELS);
                warning_layer_certified = true;
            }
        }

        if CAPTURE_ANCHORS.contains(&game_frame) {
            pending = Some((
                game_frame,
                native.frame(),
                current_draw_list,
                video::original_bitmap(&retail),
            ));
        }

        // Match the original completed bitmap to its own actual scanout.
        // This must not choose a raster by comparing against native pixels.
        // TRANS.ASM starts transferring the previous bitmap before strategies;
        // IRQ.ASM irqbit3 exposes it only after the aperture work permits the
        // buffer swap. A fixed one-update scanout delay is therefore invalid.
        while let Some((_, bitmap, _)) = pending_video.front() {
            let Some(index) = completed_rasters
                .iter()
                .position(|raster| video::displays_original_bitmap(raster, bitmap))
            else {
                break;
            };
            let raster = completed_rasters.drain(..=index).last().unwrap();
            let (scene, bitmap, native_rgb) = pending_video.pop_front().unwrap();
            let retail_rgb = video::completed_rgb(&raster);
            let difference = compare_source_rgb(
                u64::from(scene),
                raster.video_frame,
                &retail_rgb,
                &native_rgb,
            )
            .expect("compare launch anchor video");
            if let Some(mesen) = &mesen {
                let independent = mesen.settled_original_bitmap(&bitmap).unwrap_or_else(|| {
                    panic!("Mesen did not display the complete original bitmap for scene {scene}")
                });
                assert_eq!(
                    compare_source_rgb(
                        u64::from(scene),
                        independent.video_frame,
                        &independent.rgb,
                        &native_rgb
                    )
                    .unwrap(),
                    None,
                    "independent Mesen launch scene {scene}"
                );
                eprintln!(
                    "launch_mesen scene={scene} original_scanout={} compared_pixels={}",
                    independent.video_frame,
                    SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT
                );
            }
            if std::env::var_os("SF1_LAUNCH_DIAGNOSTIC").is_some() {
                write_source_rgb_ppm(
                    format!("/tmp/sf1-launch-scene-{scene}-retail.ppm"),
                    &retail_rgb,
                )
                .unwrap();
                write_source_rgb_ppm(
                    format!("/tmp/sf1-launch-scene-{scene}-native.ppm"),
                    &native_rgb,
                )
                .unwrap();
            }
            if first_video_divergence.is_none() {
                first_video_divergence = difference;
            }
            eprintln!(
                "launch_video scene={scene} original_scanout={} compared_pixels={}",
                raster.video_frame,
                SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT
            );
            certified.insert(scene);
        }
        if pending_video.is_empty() {
            completed_rasters.clear();
        }
        if certified.len() == COMPOSED_ANCHORS.len() && warning_layer_certified {
            renderer.shutdown();
            assert_eq!(certified, COMPOSED_ANCHORS.into_iter().collect());
            assert_eq!(first_video_divergence, None, "composed launch video");
            return;
        }
    }

    panic!("launch video anchors were not all reached: {certified:?}");
}
