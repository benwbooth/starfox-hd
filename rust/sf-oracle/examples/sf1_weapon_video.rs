//! Fast native/captured-retail source-video comparison for the first laser.

mod support;

use sf_difftest::{
    compare_source_rgb, read_source_rgb_ppm, write_source_rgb_ppm, SourceVideoDivergence,
};
use sf_game::shell::{FrameSnapshot, GameState, GameplayEntryPhase};
use sf_render::draw_list::DrawListEntry;
use sf_render::renderer::{
    config_from_repo_root, FrameInputs, GameState as RenderGameState, Renderer,
};

const SOURCE_WIDTH: i32 = 256;
const SOURCE_HEIGHT: i32 = 224;
const RETAIL_SCREEN_TOP: usize = 0;

fn retail_video_frame(directory: &std::path::Path, game_frame: u16) -> u64 {
    let Ok(metadata) =
        std::fs::read_to_string(directory.join(format!("sf1_weapon_game_{game_frame:03}.txt")))
    else {
        return u64::from(game_frame);
    };
    metadata
        .lines()
        .find_map(|line| line.strip_prefix("retail_video_frame="))
        .and_then(|value| value.parse().ok())
        .expect("decimal retail weapon video frame")
}

fn retail_video_path(directory: &std::path::Path, game_frame: u16) -> std::path::PathBuf {
    let scenario_capture = directory.join(format!("sf1_weapon_game_{game_frame:03}.ppm"));
    if scenario_capture.exists() {
        scenario_capture
    } else {
        directory.join(format!("retail-{game_frame:03}.ppm"))
    }
}

fn projected_draw_points(frame: &FrameSnapshot, draw: &DrawListEntry) -> Option<Vec<[i16; 2]>> {
    let shape_id = sf_render::shapes::resolve_shape_word(draw.shape_id);
    let metrics = sf_core::sf1_shape_metrics::sf1_shape_metrics(shape_id)?;
    let shape = sf_render::shape_data::SHAPE_DATA
        .iter()
        .find(|entry| entry.shape_id == shape_id)?;
    let vertices = if shape.animation_frames.is_empty() {
        shape.vertices
    } else {
        shape.animation_frames[usize::from(draw.anim_frame) % shape.animation_frames.len()]
    };
    let camera = frame.camera;
    Some(
        sf_render::source_projection::project_shape(
            vertices,
            shape.reflected_pair_starts,
            metrics.coordinate_shift,
            sf_render::source_projection::SourcePose {
                world_position: [
                    (draw.x >> 16) as i16,
                    (draw.y >> 16) as i16,
                    (draw.z >> 16) as i16,
                ],
                rotation: [draw.rx as u8, draw.ry as u8, draw.rz as u8],
                view_position: [
                    (camera.x >> 16) as i16,
                    (camera.y >> 16) as i16,
                    (camera.z >> 16) as i16,
                ],
                view_rotation: camera.rotation,
            },
        )
        .points
        .into_iter()
        .map(|point| [point.x, point.y])
        .collect(),
    )
}

fn render_diagnostic_layer(
    renderer: &mut Renderer,
    native: &sf_game::shell::Shell,
    inputs: &FrameInputs<'_>,
    draws: &[DrawListEntry],
) -> Vec<u8> {
    let camera = native.frame().camera;
    renderer
        .transform
        .set_camera_fine(camera.x, camera.y, camera.z, camera.rotation);
    if camera.snap {
        renderer.transform.snap_camera();
    }
    renderer.begin_frame();
    renderer.submit(&[], draws, 1.0, inputs);
    renderer.end_frame();
    renderer.read_pixels_rgb()
}

fn main() {
    let retail_directory: std::path::PathBuf = std::env::var_os("SF1_WEAPON_RETAIL_DIR")
        .expect("SF1_WEAPON_RETAIL_DIR must identify a logic-aligned retail capture")
        .into();
    let dump_directory: Option<std::path::PathBuf> =
        std::env::var_os("SF1_WEAPON_VIDEO_DUMP_DIR").map(Into::into);
    if let Some(directory) = dump_directory.as_ref() {
        std::fs::create_dir_all(directory).expect("create weapon video dump directory");
    }
    let dump_all = std::env::var_os("SF1_WEAPON_VIDEO_DUMP_ALL").is_some();
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let mut renderer = Renderer::new_headless(
        SOURCE_WIDTH,
        SOURCE_HEIGHT,
        &config_from_repo_root(repo_root),
    )
    .expect("headless weapon renderer");
    let mut native = support::configured_shell();
    let mut pending_scene: Option<(u16, FrameSnapshot, Vec<DrawListEntry>)> = None;
    let mut certified_updates = 0u32;
    let mut first_divergence: Option<SourceVideoDivergence> = None;
    for tick in 0..=support::WEAPON_TRACE_END_TICK {
        let input = support::weapon_video_input(&native, tick);
        native.tick(input);
        let active = native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let game_frame = native.game.vars.gameframe;
        let current_draw_list: Vec<_> = native
            .draw_list()
            .iter()
            .map(support::render_entry)
            .collect();
        if let Some((pending_game_frame, pending_frame, pending_draw_list)) = pending_scene.take() {
            assert_eq!(
                game_frame,
                pending_game_frame + 1,
                "weapon presentation phase"
            );
            let diagnostic_game_frame = std::env::var("SF1_WEAPON_VIDEO_DIAGNOSTIC_GAME_FRAME")
                .ok()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(support::WEAPON_VIDEO_CAPTURE_FIRST_GAME_FRAME);
            if pending_game_frame == diagnostic_game_frame
                && std::env::var_os("SF1_WEAPON_VIDEO_DIAGNOSTICS").is_some()
            {
                println!(
                    "native_scene_draws={:#?}",
                    support::native_source_draws(&native)
                );
                println!(
                    "native_source_projections={:#?}",
                    support::native_source_projections(&native)
                );
                if let Some(directory) = dump_directory.as_ref() {
                    let frame = native.frame();
                    let full_inputs = support::playing_frame_inputs(&frame);
                    let mut scene_inputs = full_inputs.clone();
                    scene_inputs.game_state = RenderGameState::Boot;
                    scene_inputs.point_pixels = &[];
                    let mut diagnostic_renderer = Renderer::new_headless(
                        SOURCE_WIDTH,
                        SOURCE_HEIGHT,
                        &config_from_repo_root(repo_root),
                    )
                    .expect("headless weapon diagnostic renderer");
                    let background_rgb = render_diagnostic_layer(
                        &mut diagnostic_renderer,
                        &native,
                        &scene_inputs,
                        &[],
                    );
                    write_source_rgb_ppm(directory.join("background.ppm"), &background_rgb)
                        .expect("write weapon background diagnostic");

                    let mut point_inputs = scene_inputs.clone();
                    point_inputs.point_pixels = full_inputs.point_pixels;
                    let point_rgb = render_diagnostic_layer(
                        &mut diagnostic_renderer,
                        &native,
                        &point_inputs,
                        &[],
                    );
                    write_source_rgb_ppm(directory.join("background-points.ppm"), &point_rgb)
                        .expect("write weapon point-field diagnostic");

                    let no_object_rgb = render_diagnostic_layer(
                        &mut diagnostic_renderer,
                        &native,
                        &full_inputs,
                        &[],
                    );
                    write_source_rgb_ppm(directory.join("no-objects.ppm"), &no_object_rgb)
                        .expect("write weapon non-object diagnostic");

                    for (order, draw) in current_draw_list.iter().enumerate() {
                        let object_rgb = render_diagnostic_layer(
                            &mut diagnostic_renderer,
                            &native,
                            &scene_inputs,
                            std::slice::from_ref(draw),
                        );
                        write_source_rgb_ppm(
                            directory
                                .join(format!("object-{order:02}-shape-{:03}.ppm", draw.shape_id)),
                            &object_rgb,
                        )
                        .expect("write weapon object diagnostic");
                    }
                }
            }
            let presentation_frame = native.frame();
            let native_rgb = support::render_presentation_aligned_source_frame(
                &pending_frame,
                &presentation_frame,
                &pending_draw_list,
                &mut renderer,
            );
            let retail_rgb = read_source_rgb_ppm(
                retail_video_path(&retail_directory, pending_game_frame),
                RETAIL_SCREEN_TOP,
            )
            .expect("read retail weapon source frame");
            let divergence = compare_source_rgb(
                u64::from(pending_game_frame - support::WEAPON_VIDEO_CAPTURE_FIRST_GAME_FRAME),
                retail_video_frame(&retail_directory, pending_game_frame),
                &retail_rgb,
                &native_rgb,
            )
            .expect("compare weapon source frame");
            if std::env::var_os("SF1_WEAPON_VIDEO_DIVERGENCE_CENSUS").is_some()
                && divergence.is_some()
                && first_divergence.is_none()
            {
                let source_indices = renderer.source_bitmap_indices();
                let source_rgba = renderer.source_bitmap_rgba();
                let source_owners = renderer.source_bitmap_owners();
                let source_faces = renderer.source_bitmap_faces();
                let points = pending_frame
                    .point_pixels
                    .iter()
                    .map(|pixel| (usize::from(pixel.x) + 16, usize::from(pixel.y) + 16))
                    .collect::<std::collections::HashSet<_>>();
                let differences = retail_rgb
                    .chunks_exact(3)
                    .zip(native_rgb.chunks_exact(3))
                    .enumerate()
                    .filter_map(|(index, (retail, native))| {
                        (retail != native).then(|| {
                            let x = index % SOURCE_WIDTH as usize;
                            let y = index / SOURCE_WIDTH as usize;
                            let source_index = y * SOURCE_WIDTH as usize + x;
                            (
                                x,
                                y,
                                points.contains(&(x, y)),
                                source_indices[source_index],
                                source_owners[source_index],
                                source_faces[source_index],
                                [
                                    source_rgba[source_index * 4],
                                    source_rgba[source_index * 4 + 1],
                                    source_rgba[source_index * 4 + 2],
                                    source_rgba[source_index * 4 + 3],
                                ],
                            )
                        })
                    })
                    .collect::<Vec<_>>();
                let implicated_owner_counts = differences
                    .iter()
                    .map(|difference| difference.4)
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .map(|owner| {
                        (
                            owner,
                            source_owners
                                .iter()
                                .filter(|candidate| **candidate == owner)
                                .count(),
                        )
                    })
                    .collect::<Vec<_>>();
                println!(
                    "weapon_video_divergence_census game_frame={pending_game_frame} differences={} samples={:?} implicated_owner_pixels={implicated_owner_counts:?} implicated_draws={:?}",
                    differences.len(),
                    &differences[..differences.len().min(24)],
                    pending_draw_list
                        .iter()
                        .filter(|draw| {
                            differences
                                .iter()
                                .any(|difference| difference.4 == draw.obj_id)
                        })
                        .collect::<Vec<_>>(),
                );
                for draw in pending_draw_list.iter().filter(|draw| {
                    differences
                        .iter()
                        .any(|difference| difference.4 == draw.obj_id)
                }) {
                    println!(
                        "weapon_video_implicated_projection game_frame={pending_game_frame} object={} shape={} points={:?}",
                        draw.obj_id,
                        draw.shape_id,
                        projected_draw_points(&pending_frame, draw),
                    );
                }
            }
            if dump_all {
                let directory = dump_directory
                    .as_ref()
                    .expect("SF1_WEAPON_VIDEO_DUMP_ALL requires SF1_WEAPON_VIDEO_DUMP_DIR");
                write_source_rgb_ppm(
                    directory.join(format!("native-{pending_game_frame:03}.ppm")),
                    &native_rgb,
                )
                .expect("write native weapon frame census");
            }
            if first_divergence.is_none() {
                if divergence.is_some() {
                    if let Some(directory) = dump_directory.as_ref() {
                        write_source_rgb_ppm(
                            directory.join(format!("retail-{pending_game_frame:03}.ppm")),
                            &retail_rgb,
                        )
                        .expect("write retail weapon diagnostic");
                        write_source_rgb_ppm(
                            directory.join(format!("native-{pending_game_frame:03}.ppm")),
                            &native_rgb,
                        )
                        .expect("write native weapon diagnostic");
                    }
                }
                first_divergence = divergence;
            }
            certified_updates += 1;
        }
        if active
            && (support::WEAPON_VIDEO_CAPTURE_FIRST_GAME_FRAME
                ..=support::WEAPON_VIDEO_CAPTURE_LAST_GAME_FRAME)
                .contains(&game_frame)
        {
            pending_scene = Some((game_frame, native.frame(), current_draw_list));
        }
    }

    match &first_divergence {
        Some(divergence) => println!(
            "sf1_weapon_video certified_updates={} first_divergence={} retail_video_frame={} differing_pixels={} first_position={},{} retail_color={:?} native_color={:?}",
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
            "sf1_weapon_video certified_updates={certified_updates} first_divergence=none"
        ),
    }
    assert_eq!(
        certified_updates,
        u32::from(
            support::WEAPON_VIDEO_CAPTURE_LAST_GAME_FRAME
                - support::WEAPON_VIDEO_CAPTURE_FIRST_GAME_FRAME
                + 1,
        ),
        "weapon video certification duration"
    );
    assert_eq!(
        first_divergence, None,
        "authoritative retail weapon video diverged"
    );
}
