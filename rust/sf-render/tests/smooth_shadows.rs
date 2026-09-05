//! Headless regression for the default HD smooth ground-shadow pass.

use std::path::PathBuf;

use sf_render::draw_list::{DrawListEntry, ShadowStyle, DL_FLAG_SHADOW, DL_FLAG_VISIBLE};
use sf_render::gpu::Vertex3;
use sf_render::renderer::GameState;
use sf_render::renderer::{config_from_repo_root, FrameInputs, Renderer};
use sf_render::shapes::SHAPE_MYSHIP_4;

const WIDTH: i32 = 320;
const HEIGHT: i32 = 224;
const FLOOR_HALF_WIDTH: f32 = 800.0;
const FLOOR_NEAR_Z: f32 = 50.0;
const FLOOR_FAR_Z: f32 = 900.0;
const ARWING_Y: i32 = -40;
const ARWING_Z: i32 = 200;
const CAMERA_Y: i32 = -100;
const CAMERA_Z: i32 = -300;
const CAMERA_PITCH: i16 = -12;
const FLOOR_COLOR: [f32; 4] = [0.65, 0.65, 0.65, 1.0];
const MINIMUM_SOLID_BLOCKS: usize = 4;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn render(style: ShadowStyle) -> Vec<u8> {
    let mut config = config_from_repo_root(&repo_root());
    config.shadow_style = style;
    let mut renderer = Renderer::new_headless(WIDTH, HEIGHT, &config)
        .expect("GPU required for smooth-shadow regression");

    // The neutral flat quad supplies a lit opaque surface; the Arwing is above
    // it and its projected mesh shadow lands on that surface. The ship itself
    // remains an ordinary gameplay draw-list entry, exercising the same pass
    // as the running game.
    let arwing = DrawListEntry {
        shape_id: SHAPE_MYSHIP_4,
        y: ARWING_Y << 16,
        z: ARWING_Z << 16,
        flags: DL_FLAG_VISIBLE | DL_FLAG_SHADOW,
        obj_id: 2,
        ..Default::default()
    };
    renderer
        .transform
        .set_camera(0, CAMERA_Y << 16, CAMERA_Z << 16, CAMERA_PITCH, 0, 0);
    renderer.begin_frame();
    let floor = [
        Vertex3 {
            pos: [-FLOOR_HALF_WIDTH, 0.0, FLOOR_NEAR_Z],
        },
        Vertex3 {
            pos: [FLOOR_HALF_WIDTH, 0.0, FLOOR_NEAR_Z],
        },
        Vertex3 {
            pos: [FLOOR_HALF_WIDTH, 0.0, FLOOR_FAR_Z],
        },
        Vertex3 {
            pos: [-FLOOR_HALF_WIDTH, 0.0, FLOOR_NEAR_Z],
        },
        Vertex3 {
            pos: [FLOOR_HALF_WIDTH, 0.0, FLOOR_FAR_Z],
        },
        Vertex3 {
            pos: [-FLOOR_HALF_WIDTH, 0.0, FLOOR_FAR_Z],
        },
    ];
    let identity = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    renderer.gpu.push_flat_tris(
        &floor,
        renderer.transform.projection(),
        renderer.transform.view(),
        &identity,
        FLOOR_COLOR,
    );
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        scene_style: sf_core::scene::SceneStyle::default(),
        ..FrameInputs::default()
    };
    renderer.submit(&[arwing], &[arwing], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    renderer.shutdown();
    pixels
}

#[test]
fn smooth_shadow_changes_lit_floor_without_retail_checkerboard() {
    let smooth = render(ShadowStyle::default());
    let disabled = render(ShadowStyle::Disabled);

    let mut changed = 0;
    let mut darkened = vec![false; (WIDTH * HEIGHT) as usize];
    for (pixel_index, (smooth_px, disabled_px)) in smooth
        .chunks_exact(3)
        .zip(disabled.chunks_exact(3))
        .enumerate()
    {
        if smooth_px != disabled_px {
            assert!(
                (0..3).all(|channel| smooth_px[channel] <= disabled_px[channel]),
                "shadow must darken the underlying surface"
            );
            changed += 1;
            darkened[pixel_index] = true;
        }
    }
    assert!(changed > 0, "smooth shadow did not alter the lit floor");
    // A checkerboard leaves alternating pixels untouched. The smooth pass
    // must cover solid 2-by-2 blocks on the ground, not a parity mask.
    let mut solid_blocks = 0;
    for y in 0..HEIGHT as usize - 1 {
        for x in 0..WIDTH as usize - 1 {
            let offset = y * WIDTH as usize + x;
            if [
                offset,
                offset + 1,
                offset + WIDTH as usize,
                offset + WIDTH as usize + 1,
            ]
            .into_iter()
            .all(|index| darkened[index])
            {
                solid_blocks += 1;
            }
        }
    }
    assert!(
        solid_blocks >= MINIMUM_SOLID_BLOCKS,
        "smooth shadow changed only isolated/checker pixels ({changed})"
    );
}
