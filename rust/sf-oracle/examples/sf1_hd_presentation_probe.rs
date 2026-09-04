//! Native SF1 presentation capture probe.
//!
//! This is a diagnostic capture of the native renderer's completed-scene
//! queue, not a source-resolution or retail-parity oracle.

mod support;

use sf_game::presentation::SourcePresentationQueue;
use sf_game::shell::{GameState, GameplayEntryPhase};
use sf_oracle::sf1_input::{corneria_attack_carrier_input, corneria_front_end_input};
use sf_render::draw_list::{DrawListEntry as RenderDrawListEntry, ShadowStyle};
use sf_render::renderer::{config_from_repo_root, Renderer};
use std::io::Write;
use std::path::{Path, PathBuf};

const WIDTH: i32 = 1_280;
const HEIGHT: i32 = 720;
const MAX_TICKS: u32 = 4_000;
const PHASES: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
const DEFAULT_FIRST_FRAME: u16 = 100;
const DEFAULT_LAST_FRAME: u16 = 130;

fn ppm(path: &Path, rgb: &[u8]) {
    let mut bytes = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    bytes.extend_from_slice(rgb);
    std::fs::write(path, bytes).expect("write PPM");
}

fn main() {
    let output = PathBuf::from(
        std::env::var_os("SF1_HD_PRESENTATION_OUT_DIR")
            .expect("SF1_HD_PRESENTATION_OUT_DIR is required"),
    );
    std::fs::create_dir_all(&output).expect("create output directory");
    let first = std::env::var("SF1_HD_PRESENTATION_FIRST_FRAME")
        .ok()
        .map(|v| v.parse().expect("first frame is decimal"))
        .unwrap_or(DEFAULT_FIRST_FRAME);
    let last = std::env::var("SF1_HD_PRESENTATION_LAST_FRAME")
        .ok()
        .map(|v| v.parse().expect("last frame is decimal"))
        .unwrap_or(DEFAULT_LAST_FRAME);
    assert!(first <= last, "presentation frame range is reversed");

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let mut config = config_from_repo_root(root);
    // The oracle helper deliberately defaults to retail checkerboards; this
    // probe instead uses the shipping application's default HD presentation.
    config.shadow_style = ShadowStyle::Disabled;
    let attack = std::env::var_os("SF1_HD_PRESENTATION_ATTACK").is_some();
    let mut renderer = Renderer::new_headless(WIDTH, HEIGHT, &config).expect("headless renderer");
    let mut shell = support::configured_shell();
    let mut queue = SourcePresentationQueue::<Vec<RenderDrawListEntry>>::new();
    let mut presented_current: Vec<RenderDrawListEntry> = Vec::new();
    let mut presented_frame = shell.frame();
    let mut previous_presented_frame = presented_frame.clone();
    let mut tsv = std::io::BufWriter::new(
        std::fs::File::create(output.join("scenes.tsv")).expect("create TSV"),
    );
    writeln!(tsv, "gameframe\tscene_camera\tpresentation_camera\tplayer_view_mode\tdraw_entries\tpoint_pixels\tphase\tfile")
        .expect("write TSV header");
    let mut captures = 0usize;

    for tick in 0..MAX_TICKS {
        let active = shell.state() == GameState::Playing
            && shell.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let input = if active && attack {
            corneria_attack_carrier_input(shell.game.vars.gameframe)
        } else if active {
            0
        } else {
            corneria_front_end_input(tick)
        };
        shell.tick(input);
        let current = shell.frame();
        if current.gameplay_entry_phase != GameplayEntryPhase::ActiveLevel {
            queue.reset();
            continue;
        }
        let draw: Vec<_> = shell
            .draw_list()
            .iter()
            .map(support::render_entry)
            .collect();
        let Some(presented) = queue.advance(current.clone(), draw) else {
            continue;
        };
        previous_presented_frame.clone_from(&presented_frame);
        let scene_camera = presented.scene.camera;
        let snap_scene = presented.snap_scene;
        presented_frame = presented.frame();
        let mut previous = presented_current.clone();
        presented_current = presented.content.clone();
        renderer.advance_background_offset_tables(
            presented_frame.bg2_vertical_offsets,
            presented_frame.bg2_horizontal_offsets,
        );
        renderer.transform.set_camera_fine(
            scene_camera.x,
            scene_camera.y,
            scene_camera.z,
            scene_camera.rotation,
        );
        if scene_camera.snap || snap_scene {
            renderer.transform.snap_camera();
            renderer.snap_background_offset_tables();
            previous = presented_current.clone();
            previous_presented_frame
                .point_pixels
                .clone_from(&presented_frame.point_pixels);
        }
        if !(first..=last).contains(&presented.scene.gameframe) {
            continue;
        }
        let aligned = sf_game::presentation::compose_source_presentation(
            &presented.scene,
            &presented.presentation,
        );
        let mut inputs = support::playing_frame_inputs(&aligned);
        inputs.source_resolution = false;
        inputs.source_background_pitch = None;
        inputs.source_scene_camera = None;
        inputs.point_pixels = &presented_frame.point_pixels;
        inputs.previous_point_pixels = Some(&previous_presented_frame.point_pixels);
        let safe_frame = presented.scene.gameframe;
        for (phase_index, alpha) in PHASES.into_iter().enumerate() {
            renderer.begin_frame();
            renderer.submit(&previous, &presented_current, alpha, &inputs);
            renderer.end_frame();
            let file = format!("scene-{safe_frame:04}-phase-{phase_index}.ppm");
            ppm(&output.join(&file), &renderer.read_pixels_rgb());
            writeln!(
                tsv,
                "{safe_frame}\t{:?}\t{:?}\t{:?}\t{}\t{}\t{alpha}\t{file}",
                presented.scene.camera,
                presented.presentation.camera,
                presented.scene.player_view_mode,
                presented.content.len(),
                presented.scene.point_pixels.len()
            )
            .expect("write TSV row");
            captures += 1;
        }
        if safe_frame == last {
            break;
        }
    }
    renderer.shutdown();
    tsv.flush().expect("flush TSV");
    assert_eq!(
        captures,
        usize::from(last - first + 1) * PHASES.len(),
        "every requested interval must have all presentation phases"
    );
    eprintln!("captured {captures} phase images in {}", output.display());
}
