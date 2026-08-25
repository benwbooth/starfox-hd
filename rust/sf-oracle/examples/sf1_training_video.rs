//! Strict source-resolution video comparison for the complete Training loop.

mod support;

use sf_core::pad;
use sf_difftest::{
    compare_source_rgb, read_source_rgb_ppm, write_source_rgb_ppm, SourceVideoDivergence,
};
use sf_game::shell::{FrameSnapshot, GameState, GameplayEntryPhase};
use sf_render::draw_list::DrawListEntry;
use sf_render::renderer::{config_from_repo_root, Renderer};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SOURCE_WIDTH: i32 = 256;
const SOURCE_HEIGHT: i32 = 224;
const RETAIL_SCREEN_TOP: usize = 0;
const FRONT_END_CONFIRM_CADENCE_TICKS: u32 = 60;
const FRONT_END_CONFIRM_HOLD_TICKS: u32 = 2;
const TRAINING_CONFIRM_END_TICK: u32 = 420;
const PROBE_END_TICK: u32 = 2_200;
const COMPLETE_TRAINING_FIRST_GAME_FRAME: u16 = 1;
const COMPLETE_TRAINING_LAST_GAME_FRAME: u16 = 1_758;

fn scripted_input(tick: u32) -> u16 {
    if tick <= TRAINING_CONFIRM_END_TICK
        && tick % FRONT_END_CONFIRM_CADENCE_TICKS < FRONT_END_CONFIRM_HOLD_TICKS
    {
        pad::START
    } else {
        0
    }
}

fn parse_capture_manifest(path: &Path) -> BTreeMap<u16, u64> {
    let text = std::fs::read_to_string(path).expect("read Training capture manifest");
    text.lines()
        .map(|line| {
            let fields = line
                .split_whitespace()
                .filter_map(|field| field.split_once('='))
                .collect::<BTreeMap<_, _>>();
            let game_frame = fields["scene_game_frame"]
                .parse::<u16>()
                .expect("decimal Training scene game frame");
            let retail_video_frame = fields["retail_video_frame"]
                .parse::<u64>()
                .expect("decimal Training retail video frame");
            (game_frame, retail_video_frame)
        })
        .collect()
}

fn retail_video_path(directory: &Path, game_frame: u16) -> PathBuf {
    directory.join(format!("sf1_training_game_{game_frame:04}.ppm"))
}

fn main() {
    let retail_directory: PathBuf = std::env::var_os("SF1_TRAINING_RETAIL_DIR")
        .expect("SF1_TRAINING_RETAIL_DIR must identify a logic-aligned retail capture")
        .into();
    let retail_video_frames =
        parse_capture_manifest(&retail_directory.join("sf1_training_captures.txt"));
    let first_game_frame = std::env::var("SF1_TRAINING_VIDEO_FIRST_GAME_FRAME")
        .ok()
        .map(|value| {
            value
                .parse::<u16>()
                .expect("decimal first Training game frame")
        })
        .unwrap_or(COMPLETE_TRAINING_FIRST_GAME_FRAME);
    let last_game_frame = std::env::var("SF1_TRAINING_VIDEO_LAST_GAME_FRAME")
        .ok()
        .map(|value| {
            value
                .parse::<u16>()
                .expect("decimal last Training game frame")
        })
        .unwrap_or(COMPLETE_TRAINING_LAST_GAME_FRAME);
    assert!(
        first_game_frame >= COMPLETE_TRAINING_FIRST_GAME_FRAME
            && first_game_frame <= last_game_frame
            && last_game_frame <= COMPLETE_TRAINING_LAST_GAME_FRAME,
        "Training video range"
    );
    let dump_directory: Option<PathBuf> =
        std::env::var_os("SF1_TRAINING_VIDEO_DUMP_DIR").map(Into::into);
    if let Some(directory) = dump_directory.as_ref() {
        std::fs::create_dir_all(directory).expect("create Training video dump directory");
    }
    let dump_all = std::env::var_os("SF1_TRAINING_VIDEO_DUMP_ALL").is_some();
    let trace_presentation = std::env::var_os("SF1_TRAINING_VIDEO_TRACE_PRESENTATION").is_some();

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let mut renderer = Renderer::new_headless(
        SOURCE_WIDTH,
        SOURCE_HEIGHT,
        &config_from_repo_root(repo_root),
    )
    .expect("headless Training renderer");
    let mut native = support::configured_shell();
    let mut pending_scene: Option<(u16, FrameSnapshot, Vec<DrawListEntry>)> = None;
    let mut certified_updates = 0u32;
    let mut first_divergence: Option<SourceVideoDivergence> = None;

    for tick in 0..=PROBE_END_TICK {
        native.tick(scripted_input(tick));
        let active = native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let game_frame = native.game.vars.gameframe;
        let current_draw_list = native
            .draw_list()
            .iter()
            .map(support::render_entry)
            .collect::<Vec<_>>();

        if let Some((pending_game_frame, pending_frame, pending_draw_list)) = pending_scene.take() {
            assert_eq!(
                game_frame,
                pending_game_frame + 1,
                "Training presentation phase"
            );
            let presentation_frame = if std::env::var_os("SF1_TRAINING_SCENE_PRESENTATION")
                .is_some()
            {
                &pending_frame
            } else {
                &native.frame()
            };
            let mut source_scene = pending_frame.clone();
            if std::env::var_os("SF1_TRAINING_DIAGNOSTIC_PREVIOUS_BRIGHTNESS").is_some() {
                source_scene.display_brightness = source_scene.display_brightness.saturating_sub(1);
            }
            if let Some(delta) = std::env::var("SF1_TRAINING_DIAGNOSTIC_VERTICAL_OFFSET_DELTA")
                .ok()
                .map(|value| value.parse::<i16>().expect("signed vertical offset delta"))
            {
                source_scene.bg2_vertical_offsets = source_scene
                    .bg2_vertical_offsets
                    .map(|offsets| offsets.map(|offset| offset.wrapping_add(delta)));
            }
            let native_rgb = support::render_presentation_aligned_source_frame(
                &source_scene,
                presentation_frame,
                if std::env::var_os("SF1_TRAINING_ISOLATE_BACKGROUND").is_some() {
                    &[]
                } else {
                    &pending_draw_list
                },
                &mut renderer,
            );
            if dump_all {
                let directory = dump_directory
                    .as_ref()
                    .expect("SF1_TRAINING_VIDEO_DUMP_ALL requires a dump directory");
                std::fs::write(
                    directory.join(format!("native-bitmap-{pending_game_frame:04}.bin")),
                    renderer.source_bitmap_indices(),
                )
                .expect("write native Training source bitmap census");
                std::fs::write(
                    directory.join(format!("native-owners-{pending_game_frame:04}.bin")),
                    renderer
                        .source_bitmap_owners()
                        .iter()
                        .flat_map(|owner| owner.to_le_bytes())
                        .collect::<Vec<_>>(),
                )
                .expect("write native Training source owner census");
                std::fs::write(
                    directory.join(format!("native-faces-{pending_game_frame:04}.bin")),
                    renderer
                        .source_bitmap_faces()
                        .iter()
                        .flat_map(|face| face.to_le_bytes())
                        .collect::<Vec<_>>(),
                )
                .expect("write native Training source face census");
            }
            if std::env::var_os("SF1_TRAINING_VIDEO_TRACE_DRAWS").is_some() {
                println!(
                    "Training source draws scene={} draws={pending_draw_list:?}",
                    pending_game_frame
                );
                for draw in &pending_draw_list {
                    let shape = sf_render::shapes::resolve_shape_word(draw.shape_id);
                    let Some(metrics) = sf_core::sf1_shape_metrics::sf1_shape_metrics(shape) else {
                        continue;
                    };
                    let Some(shape_data) = sf_render::shape_data::SHAPE_DATA
                        .iter()
                        .find(|entry| entry.shape_id == shape)
                    else {
                        continue;
                    };
                    let projected = sf_render::source_projection::project_shape(
                        shape_data.vertices,
                        shape_data.reflected_pair_starts,
                        metrics.coordinate_shift,
                        sf_render::source_projection::SourcePose {
                            world_position: [
                                (draw.x >> 16) as i16,
                                (draw.y >> 16) as i16,
                                (draw.z >> 16) as i16,
                            ],
                            rotation: [draw.rx as u8, draw.ry as u8, draw.rz as u8],
                            view_position: [
                                (pending_frame.camera.x >> 16) as i16,
                                (pending_frame.camera.y >> 16) as i16,
                                (pending_frame.camera.z >> 16) as i16,
                            ],
                            view_rotation: pending_frame.camera.rotation,
                        },
                    );
                    println!(
                        "Training source projection scene={} object={} shape={} points={:?} face_visibility={:?}",
                        pending_game_frame,
                        draw.obj_id,
                        shape,
                        projected.points,
                        shape_data
                            .faces
                            .iter()
                            .map(|face| face.visibility_vertices.map(|indices| {
                                sf_render::source_projection::face_is_visible(
                                    &projected.points,
                                    indices,
                                )
                            }))
                            .collect::<Vec<_>>(),
                    );
                }
            }
            if trace_presentation {
                let presentation = native.frame();
                println!(
                    "Training scene={} presentation={} stage_banner={:?} scene_camera={:?} presentation_camera={:?} scene_bg2={} presentation_bg2={} scene_voffset={:?} presentation_voffset={:?} scene_style={:?} presentation_style={:?} scene_palfade={} presentation_palfade={} scene_wipe={:?} presentation_wipe={:?} scene_brightness={} presentation_brightness={} black_subtraction={} scene_forced_blank={} presentation_forced_blank={}",
                    pending_game_frame,
                    presentation.gameframe,
                    pending_frame.stage_banner,
                    pending_frame.camera,
                    presentation.camera,
                    pending_frame.bg2_xscroll,
                    presentation.bg2_xscroll,
                    pending_frame.bg2_vertical_offsets.map(|offsets| offsets[0]),
                    presentation.bg2_vertical_offsets.map(|offsets| offsets[0]),
                    pending_frame.scene_style,
                    presentation.scene_style,
                    pending_frame.palfade_num,
                    presentation.palfade_num,
                    pending_frame.screen_wipe,
                    presentation.screen_wipe,
                    pending_frame.display_brightness,
                    presentation.display_brightness,
                    presentation.display_black_subtraction,
                    pending_frame.display_forced_blank,
                    presentation.display_forced_blank,
                );
            }
            let retail_rgb = read_source_rgb_ppm(
                retail_video_path(&retail_directory, pending_game_frame),
                RETAIL_SCREEN_TOP,
            )
            .expect("read retail Training source frame");
            let retail_video_frame = retail_video_frames
                .get(&pending_game_frame)
                .copied()
                .expect("Training capture manifest game frame");
            let divergence = compare_source_rgb(
                u64::from(pending_game_frame - first_game_frame),
                retail_video_frame,
                &retail_rgb,
                &native_rgb,
            )
            .expect("compare Training source frame");
            if std::env::var_os("SF1_TRAINING_VIDEO_DIVERGENCE_CENSUS").is_some()
                && divergence.is_some()
            {
                let source_owners = renderer.source_bitmap_owners();
                let source_faces = renderer.source_bitmap_faces();
                let source_indices = renderer.source_bitmap_indices();
                let mut owner_counts = BTreeMap::<u16, usize>::new();
                let mut face_counts = BTreeMap::<u16, usize>::new();
                let mut samples = Vec::new();
                for (index, (retail, native)) in retail_rgb
                    .chunks_exact(3)
                    .zip(native_rgb.chunks_exact(3))
                    .enumerate()
                {
                    if retail != native {
                        *owner_counts.entry(source_owners[index]).or_default() += 1;
                        *face_counts.entry(source_faces[index]).or_default() += 1;
                        if pending_game_frame == first_game_frame && samples.len() < 64 {
                            samples.push((
                                index % SOURCE_WIDTH as usize,
                                index / SOURCE_WIDTH as usize,
                                retail,
                                native,
                                source_indices[index],
                                source_owners[index],
                                source_faces[index],
                            ));
                        }
                    }
                }
                println!(
                    "Training divergence scene={} owner_counts={:?} face_counts={:?} samples={:?} draws={:?}",
                    pending_game_frame, owner_counts, face_counts, samples, pending_draw_list,
                );
            }
            if first_divergence.is_none() {
                if divergence.is_some() {
                    if let Some(directory) = dump_directory.as_ref() {
                        write_source_rgb_ppm(
                            directory.join(format!("retail-{pending_game_frame:04}.ppm")),
                            &retail_rgb,
                        )
                        .expect("write retail Training divergence");
                        write_source_rgb_ppm(
                            directory.join(format!("native-{pending_game_frame:04}.ppm")),
                            &native_rgb,
                        )
                        .expect("write native Training divergence");
                    }
                }
                first_divergence = divergence;
            }
            if dump_all {
                let directory = dump_directory
                    .as_ref()
                    .expect("SF1_TRAINING_VIDEO_DUMP_ALL requires a dump directory");
                write_source_rgb_ppm(
                    directory.join(format!("native-{pending_game_frame:04}.ppm")),
                    &native_rgb,
                )
                .expect("write native Training frame census");
            }
            certified_updates += 1;
        }

        if active && (first_game_frame..=last_game_frame).contains(&game_frame) {
            pending_scene = Some((game_frame, native.frame(), current_draw_list));
        }
        if certified_updates == u32::from(last_game_frame - first_game_frame + 1)
            && pending_scene.is_none()
        {
            break;
        }
    }

    match &first_divergence {
        Some(divergence) => println!(
            "sf1_training_video certified_updates={} first_divergence={} retail_video_frame={} differing_pixels={} first_position={},{} retail_color={:?} native_color={:?}",
            certified_updates,
            divergence.sequence,
            divergence.retail_video_frame,
            divergence.differing_pixels,
            divergence.first_position[0],
            divergence.first_position[1],
            divergence.retail_color,
            divergence.native_color,
        ),
        None => println!(
            "sf1_training_video certified_updates={certified_updates} first_divergence=none"
        ),
    }
    assert_eq!(
        certified_updates,
        u32::from(last_game_frame - first_game_frame + 1),
        "Training video certification duration"
    );
    assert_eq!(
        first_divergence, None,
        "authoritative retail Training video diverged"
    );
}
