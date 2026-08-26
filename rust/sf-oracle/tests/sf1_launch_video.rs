//! Strict composed-video anchors around the source-hardware-only launch
//! aperture cadence interval.

#[path = "../examples/support/mod.rs"]
mod support;

use sf_core::stage_banner::ScrambleBannerState;
use sf_difftest::{compare_source_rgb, SOURCE_FRAME_HEIGHT, SOURCE_FRAME_WIDTH};
use sf_game::shell::{FrameSnapshot, GameState, GameplayEntryPhase};
use sf_oracle::{
    load_retail_rom, PpuFrame, RetailMachine, RETAIL_BUILD_DRAWLIST_L, RETAIL_DOSTRATS,
    RETAIL_GAMEFRAME, RETAIL_SCRAMBLE_COUNT,
};
use sf_render::{
    draw_list::DrawListEntry,
    renderer::{config_from_repo_root, FrameInputs, GameState as RenderGameState, Renderer},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const RETAIL_ROM_SHA256: &str = "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548";
const WORK_RAM: u32 = 0x7E_0000;
const VIDEO_FRAMES_PER_NATIVE_TICK: u32 = 3;
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD: u32 = 240;
const COMPLETED_FRAME_ALIGNMENT_TICK: u32 = 900;
const CORNERIA_AUDIO_UPLOAD_TICK: u32 = 1_080;
const FIRST_APERTURE_ANCHOR: u16 = 7;
const POST_APERTURE_ANCHOR: u16 = 20;
const STALE_WARNING_LAYER_ANCHOR: u16 = 21;
const COMPOSED_ANCHORS: [u16; 2] = [FIRST_APERTURE_ANCHOR, POST_APERTURE_ANCHOR];
const CAPTURE_ANCHORS: [u16; 3] = [
    FIRST_APERTURE_ANCHOR,
    POST_APERTURE_ANCHOR,
    STALE_WARNING_LAYER_ANCHOR,
];
const WARNING_LEFT: usize = 72;
const WARNING_TOP: usize = 72;
const WARNING_WIDTH: usize = 128;
const WARNING_HEIGHT: usize = 16;
const WARNING_OPAQUE_PIXELS: usize = 1_671;

fn source_rgb(frame: PpuFrame) -> Vec<u8> {
    assert_eq!(frame.width, SOURCE_FRAME_WIDTH);
    assert_eq!(frame.height, SOURCE_FRAME_HEIGHT);
    frame
        .rgba
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect()
}

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
    let mut retail_level_boundary_aligned = false;
    let mut previous_retail_level_frame = None;
    let mut pending: Option<(u16, FrameSnapshot, Vec<DrawListEntry>)> = None;
    let mut certified = BTreeSet::new();
    let mut stale_warning_layer_certified = false;

    for tick in 0..=support::WEAPON_TRACE_END_TICK {
        let input = support::weapon_input(tick);
        let next_input = support::weapon_input(tick.saturating_add(1));
        let native_level_active = native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let align_completed_level_frame =
            native_level_active && tick >= COMPLETED_FRAME_ALIGNMENT_TICK;
        let mut retail_scene_draws = None;
        let mut retail_video = None;

        if align_completed_level_frame {
            if !retail_level_boundary_aligned {
                assert!(
                    retail
                        .tick_until_cpu_execution(
                            input,
                            RETAIL_DOSTRATS,
                            MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
                        )
                        .expect("initial launch gameplay boundary"),
                    "retail did not reach the initial launch gameplay boundary"
                );
                retail_level_boundary_aligned = true;
            }
            let max_video_frames = if tick == CORNERIA_AUDIO_UPLOAD_TICK {
                MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD
            } else {
                MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE
            };
            assert!(
                retail
                    .tick_until_cpu_execution(input, RETAIL_BUILD_DRAWLIST_L, max_video_frames)
                    .expect("launch draw-list boundary"),
                "retail did not complete launch draw list at tick {tick}"
            );
            retail_scene_draws = Some(support::retail_source_draws(&retail));
            assert!(
                retail
                    .tick_until_cpu_execution(next_input, RETAIL_DOSTRATS, max_video_frames)
                    .expect("next launch gameplay boundary"),
                "retail did not reach the next launch gameplay update at tick {tick}"
            );
            retail_video = Some((retail.video_frame(), source_rgb(retail.ppu_frame())));
        } else {
            retail
                .tick_video_frames(input, VIDEO_FRAMES_PER_NATIVE_TICK)
                .expect("retail launch front end");
        }

        let retail_level_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        let retail_completed_level_update = align_completed_level_frame
            || previous_retail_level_frame.is_none_or(|previous| previous != retail_level_frame);
        if !native_level_active || retail_completed_level_update {
            native.tick(input);
        }
        if native.state() != GameState::Playing
            || native.frame().gameplay_entry_phase != GameplayEntryPhase::ActiveLevel
        {
            continue;
        }
        previous_retail_level_frame = Some(retail_level_frame);

        let game_frame = native.game.vars.gameframe;
        let current_draw_list = native
            .draw_list()
            .iter()
            .map(support::render_entry)
            .collect::<Vec<_>>();
        if game_frame == FIRST_APERTURE_ANCHOR {
            assert_eq!(
                native.game.vars.scramble_count, 50,
                "Corneria source map must arm the SCRAMBLE presentation"
            );
        }
        if CAPTURE_ANCHORS.contains(&game_frame) {
            assert_eq!(
                support::native_source_draws(&native),
                *retail_scene_draws
                    .as_ref()
                    .expect("retail launch scene draw list"),
                "launch scene draws at game frame {game_frame}"
            );
        }

        if let Some((pending_game_frame, pending_frame, pending_draw_list)) = pending.take() {
            assert_eq!(game_frame, pending_game_frame + 1);
            if COMPOSED_ANCHORS.contains(&pending_game_frame) {
                let native_rgb = support::render_presentation_aligned_source_frame(
                    &pending_frame,
                    &native.frame(),
                    &pending_draw_list,
                    &mut renderer,
                );
                let (retail_video_frame, retail_rgb) = retail_video
                    .as_ref()
                    .expect("presentation-aligned retail launch frame");
                assert_eq!(
                    compare_source_rgb(
                        u64::from(pending_game_frame),
                        *retail_video_frame,
                        retail_rgb,
                        &native_rgb,
                    )
                    .expect("compare launch anchor video"),
                    None,
                    "composed launch video at game frame {pending_game_frame}"
                );
                certified.insert(pending_game_frame);
            } else {
                assert_eq!(pending_game_frame, STALE_WARNING_LAYER_ANCHOR);
                assert_eq!(
                    retail.peek8(WORK_RAM | RETAIL_SCRAMBLE_COUNT),
                    0,
                    "retail scanout warning must be an older OAM generation"
                );
                let retail_objects = retail.ppu_snapshot_obj_rgba();

                let mut isolated = FrameInputs {
                    source_resolution: true,
                    game_state: RenderGameState::Playing,
                    scramble_banner: Some(ScrambleBannerState {
                        ticks_remaining: 50,
                        game_frame: 0,
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
                stale_warning_layer_certified = true;
            }
        }

        if CAPTURE_ANCHORS.contains(&game_frame) {
            pending = Some((game_frame, native.frame(), current_draw_list));
        }
        if certified.len() == COMPOSED_ANCHORS.len() && stale_warning_layer_certified {
            renderer.shutdown();
            assert_eq!(certified, COMPOSED_ANCHORS.into_iter().collect());
            return;
        }
    }

    panic!("launch video anchors were not all reached: {certified:?}");
}
