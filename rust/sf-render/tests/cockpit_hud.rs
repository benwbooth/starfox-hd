//! Both shipping presentation modes consume the completed cockpit reticle.
use sf_core::cockpit_hud::CockpitHudState;
use sf_core::shape::SF1_SHAPE_COCKPIT_PLAYER;
use sf_render::{
    renderer::{config_from_repo_root, FrameInputs, Renderer},
    shapes::{decode_shape_palette, game_palette_bgr},
};

#[test]
fn reticle_renders_without_meshes_and_keeps_the_previous_snapshot_until_the_boundary() {
    const WIDTH: usize = 256;
    const HEIGHT: usize = 224;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = config_from_repo_root(&root);
    let mut renderer = Renderer::new_headless(WIDTH as i32, HEIGHT as i32, &config)
        .expect("GPU required for cockpit HUD presentation regression");
    let invisible_ship = renderer.shapes.get(SF1_SHAPE_COCKPIT_PLAYER).unwrap();
    assert!(invisible_ship.vertices.is_empty());
    assert!(invisible_ship.faces.is_empty());
    assert_eq!(
        renderer.shapes.shape_half_extents(SF1_SHAPE_COCKPIT_PLAYER),
        Some((36, 14, 80))
    );
    let previous = CockpitHudState {
        enabled: true,
        roll: 37,
        palette_index: 15,
        left_wing_broken: true,
        right_wing_broken: false,
    };
    let current = CockpitHudState {
        roll: 193,
        palette_index: 5,
        left_wing_broken: false,
        right_wing_broken: true,
        ..previous
    };
    for source_resolution in [true, false] {
        for (prior, next, alpha) in [
            (None, previous, 0.0),
            (Some(previous), current, 0.0),
            (Some(previous), current, 0.5),
            (Some(previous), current, 1.0),
            (Some(current), CockpitHudState::default(), 0.5),
            (Some(current), CockpitHudState::default(), 1.0),
        ] {
            let expected = if alpha < 1.0 {
                prior.unwrap_or(next)
            } else {
                next
            };
            let inputs = FrameInputs {
                source_resolution,
                cockpit_hud: next,
                previous_cockpit_hud: prior,
                ..Default::default()
            };
            renderer.begin_frame();
            renderer.submit(&[], &[], alpha, &inputs);
            renderer.end_frame();
            let palette = decode_shape_palette(game_palette_bgr(inputs.scene_style.game_palette));
            let mut indices = vec![0u8; WIDTH * HEIGHT];
            let mut pixels = vec![0u8; WIDTH * HEIGHT * 3];
            for point in expected.pixels() {
                let position = (usize::from(point.y) + 16) * WIDTH + usize::from(point.x) + 16;
                indices[position] = point.palette_index;
                let color =
                    palette[usize::from(point.palette_index)].map(|c| (c * 255.0).round() as u8);
                pixels[position * 3..position * 3 + 3].copy_from_slice(&color);
            }
            if source_resolution {
                assert_eq!(
                    renderer.source_bitmap_indices(),
                    indices,
                    "indexed alpha={alpha}"
                );
                let work = renderer.source_frame_workload();
                assert_eq!(work.lines_drawn, if expected.enabled { 8 } else { 0 });
                assert_eq!(work.line_candidates, work.lines_drawn);
                assert_eq!(work.line_writes as usize, expected.pixels().len());
            }
            let actual = renderer.read_pixels_rgb();
            let differences = actual
                .chunks_exact(3)
                .zip(pixels.chunks_exact(3))
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(differences, 0, "source={source_resolution} alpha={alpha}");
        }
    }
}
