//! Diagnose fractional-frame presentation during the Corneria launch corridor.

mod support;

use sf_game::shell::{FrameSnapshot, GameState, GameplayEntryPhase};
use sf_render::draw_list::{DrawListEntry, ShadowStyle};
use sf_render::renderer::{config_from_repo_root, Renderer};
use sf_render::shapes::{SHAPE_ALIAS_OP_0, SHAPE_ALIAS_OP_1, SHAPE_ALIAS_OP_2};
use std::path::{Path, PathBuf};

const PROBE_END_TICK: u32 = 1_500;
const OUTPUT_WIDTH: i32 = 1_280;
const OUTPUT_HEIGHT: i32 = 720;
const WORLD_FRACTIONAL_BITS: u32 = 16;
const RGB_CHANNELS: usize = 3;
const FRACTIONAL_PHASES: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
const CORRIDOR_DEPTH_TRANSITION_FRAME: u16 = 97;
const MAXIMUM_FRACTIONAL_RMSE: f64 = 0.085;

fn is_corridor(entry: &DrawListEntry) -> bool {
    matches!(
        entry.shape_id,
        SHAPE_ALIAS_OP_0 | SHAPE_ALIAS_OP_1 | SHAPE_ALIAS_OP_2
    )
}

fn write_ppm(path: &Path, rgb: &[u8]) {
    let mut bytes = format!("P6\n{OUTPUT_WIDTH} {OUTPUT_HEIGHT}\n{}\n", u8::MAX).into_bytes();
    bytes.extend_from_slice(rgb);
    std::fs::write(path, bytes).expect("write probe frame");
}

fn set_camera_pair(renderer: &mut Renderer, previous: &FrameSnapshot, current: &FrameSnapshot) {
    let previous_camera = previous.camera;
    renderer.transform.set_camera_fine(
        previous_camera.x,
        previous_camera.y,
        previous_camera.z,
        previous_camera.rotation,
    );
    renderer.transform.snap_camera();
    let current_camera = current.camera;
    renderer.transform.set_camera_fine(
        current_camera.x,
        current_camera.y,
        current_camera.z,
        current_camera.rotation,
    );
}

fn differing_pixels(left: &[u8], right: &[u8]) -> usize {
    left.chunks_exact(RGB_CHANNELS)
        .zip(right.chunks_exact(RGB_CHANNELS))
        .filter(|(left, right)| left != right)
        .count()
}

fn normalized_rgb_rmse(left: &[u8], right: &[u8]) -> f64 {
    const CHANNEL_MAX: f64 = u8::MAX as f64;

    let squared_error = left
        .iter()
        .zip(right)
        .map(|(left, right)| {
            let difference = f64::from(*left) - f64::from(*right);
            difference * difference
        })
        .sum::<f64>();
    (squared_error / left.len() as f64).sqrt() / CHANNEL_MAX
}

fn source_depth(frame: &FrameSnapshot, entry: &DrawListEntry) -> i16 {
    let camera = frame.camera;
    let relative = [
        ((entry.x.wrapping_sub(camera.x)) >> WORLD_FRACTIONAL_BITS) as i16,
        ((entry.y.wrapping_sub(camera.y)) >> WORLD_FRACTIONAL_BITS) as i16,
        ((entry.z.wrapping_sub(camera.z)) >> WORLD_FRACTIONAL_BITS) as i16,
    ];
    let matrix = sf_core::snes_trig::zxy_matrix_q15_fine(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
    );
    sf_core::snes_trig::matrix_rotate_q15(matrix, relative[0], relative[1], relative[2]).2
}

fn run_probe(first_game_frame: u16, last_game_frame: u16, output_directory: Option<PathBuf>) {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    if let Some(directory) = output_directory.as_ref() {
        std::fs::create_dir_all(directory).expect("create probe directory");
    }

    let mut config = config_from_repo_root(repository);
    config.shadow_style = ShadowStyle::Disabled;
    let mut renderer =
        Renderer::new_headless(OUTPUT_WIDTH, OUTPUT_HEIGHT, &config).expect("headless renderer");
    let mut shell = support::configured_shell();
    let mut previous_frame = shell.frame();
    let mut previous_draw_list = Vec::<DrawListEntry>::new();
    let mut corridor_updates = 0usize;
    let mut largest_fractional_change = (0usize, 0u16, 0usize);
    let mut depth_transition_verified = false;

    for tick in 0..=PROBE_END_TICK {
        shell.tick(support::weapon_input(tick));
        let current_frame = shell.frame();
        let current_draw_list = shell
            .draw_list()
            .iter()
            .map(support::render_entry)
            .collect::<Vec<_>>();
        let active = shell.state() == GameState::Playing
            && current_frame.gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let corridor_count = current_draw_list
            .iter()
            .filter(|entry| is_corridor(entry))
            .count();
        if active
            && (first_game_frame..=last_game_frame).contains(&current_frame.gameframe)
            && corridor_count != 0
            && previous_frame.gameframe + 1 == current_frame.gameframe
        {
            let mut inputs = support::playing_frame_inputs(&current_frame);
            inputs.source_resolution = false;
            inputs.source_background_pitch = None;
            inputs.source_scene_camera = None;
            let mut phase_frames = Vec::new();
            for (phase_index, phase) in FRACTIONAL_PHASES.into_iter().enumerate() {
                set_camera_pair(&mut renderer, &previous_frame, &current_frame);
                renderer.begin_frame();
                renderer.submit(&previous_draw_list, &current_draw_list, phase, &inputs);
                renderer.end_frame();
                let rgb = renderer.read_pixels_rgb();
                if let Some(directory) = output_directory.as_ref() {
                    write_ppm(
                        &directory.join(format!(
                            "corneria-{:04}-phase-{phase_index}.ppm",
                            current_frame.gameframe
                        )),
                        &rgb,
                    );
                }
                phase_frames.push(rgb);
            }
            let changes = phase_frames
                .windows(2)
                .map(|pair| differing_pixels(&pair[0], &pair[1]))
                .collect::<Vec<_>>();
            let root_mean_square_errors = phase_frames
                .windows(2)
                .map(|pair| normalized_rgb_rmse(&pair[0], &pair[1]))
                .collect::<Vec<_>>();
            if current_frame.gameframe == CORRIDOR_DEPTH_TRANSITION_FRAME {
                assert!(
                    root_mean_square_errors
                        .iter()
                        .all(|error| *error <= MAXIMUM_FRACTIONAL_RMSE),
                    "Corneria corridor fractional-frame discontinuity returned: {root_mean_square_errors:?}"
                );
                depth_transition_verified = true;
            }
            if let Some((phase_index, change)) = changes
                .iter()
                .copied()
                .enumerate()
                .max_by_key(|(_, change)| *change)
            {
                if change > largest_fractional_change.2 {
                    largest_fractional_change = (phase_index, current_frame.gameframe, change);
                }
            }
            println!(
                "gameframe={} corridor={} camera=({},{},{}) rotation={:?} fractional_changes={changes:?} fractional_rmse={root_mean_square_errors:?} segments={:?}",
                current_frame.gameframe,
                corridor_count,
                current_frame.camera.x >> WORLD_FRACTIONAL_BITS,
                current_frame.camera.y >> WORLD_FRACTIONAL_BITS,
                current_frame.camera.z >> WORLD_FRACTIONAL_BITS,
                current_frame.camera.rotation,
                current_draw_list
                    .iter()
                    .filter(|entry| is_corridor(entry))
                    .map(|entry| (
                        entry.obj_id,
                        entry.shape_id,
                        entry.x >> WORLD_FRACTIONAL_BITS,
                        entry.y >> WORLD_FRACTIONAL_BITS,
                        entry.z >> WORLD_FRACTIONAL_BITS,
                        source_depth(&current_frame, entry),
                    ))
                    .collect::<Vec<_>>(),
            );
            corridor_updates += 1;
        }
        previous_frame = current_frame;
        previous_draw_list = current_draw_list;
    }

    renderer.shutdown();
    println!(
        "corridor_updates={corridor_updates} largest_fractional_change={largest_fractional_change:?}"
    );
    if (first_game_frame..=last_game_frame).contains(&CORRIDOR_DEPTH_TRANSITION_FRAME) {
        assert!(
            depth_transition_verified,
            "Corneria depth-transition frame was not reached"
        );
    }
}

pub fn verify_corneria_depth_transition() {
    run_probe(
        CORRIDOR_DEPTH_TRANSITION_FRAME,
        CORRIDOR_DEPTH_TRANSITION_FRAME,
        None,
    );
}

#[allow(dead_code)]
fn main() {
    let output_directory = std::env::var_os("SF1_CORRIDOR_HD_DUMP_DIR").map(PathBuf::from);
    let first_game_frame = std::env::var("SF1_CORRIDOR_HD_FIRST_GAME_FRAME")
        .ok()
        .map(|value| value.parse::<u16>().expect("decimal first game frame"))
        .unwrap_or(1);
    let last_game_frame = std::env::var("SF1_CORRIDOR_HD_LAST_GAME_FRAME")
        .ok()
        .map(|value| value.parse::<u16>().expect("decimal last game frame"))
        .unwrap_or(u16::MAX);
    run_probe(first_game_frame, last_game_frame, output_directory);
}
