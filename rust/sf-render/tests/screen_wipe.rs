use sf_core::screen_wipe::{ScreenWipeKind, ScreenWipeState};
use sf_render::renderer::{config_from_repo_root, FrameInputs, GameState, Renderer};

const WIDTH: i32 = 224;
const HEIGHT: i32 = 192;
const SOURCE_WIDTH: f32 = 256.0;
const SOURCE_HEIGHT: f32 = 224.0;
const HALF_REVEAL_HEIGHT: usize = HEIGHT as usize / 2;
const VISIBLE_TEST_BACKGROUND: u16 = 4;
const MASK_COLOR: [u8; 3] = [0, 0, 0];

fn render(wipe: ScreenWipeState) -> Option<Vec<u8>> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = config_from_repo_root(&root);
    let mut renderer = match Renderer::new_headless(WIDTH, HEIGHT, &config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping screen-wipe render check: no wgpu adapter ({error})");
            return None;
        }
    };
    let inputs = FrameInputs {
        game_state: GameState::Playing,
        newmap: 1,
        currentbg: VISIBLE_TEST_BACKGROUND,
        screen_wipe: wipe,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    renderer.shutdown();
    Some(pixels)
}

#[test]
fn horizontal_reveal_preserves_canvas_aspect_and_masks_the_outer_surface() {
    let mut closed = ScreenWipeState::inactive();
    closed.begin(ScreenWipeKind::HorizontalReveal);
    let Some(closed_pixels) = render(closed) else {
        return;
    };
    assert!(closed_pixels
        .chunks_exact(3)
        .all(|pixel| pixel == [0, 0, 0]));

    let mut half_open = closed;
    for _ in 0..6 {
        assert!(half_open.advance());
    }
    let half_open_pixels = render(half_open).expect("same adapter remains available");
    let revealed = half_open_pixels
        .chunks_exact(3)
        .filter(|pixel| **pixel != MASK_COLOR)
        .count();
    let canvas_width = (HEIGHT as f32 * SOURCE_WIDTH / SOURCE_HEIGHT).round() as usize;
    let canvas_left = (WIDTH as usize - canvas_width) / 2;
    let expected_revealed = canvas_width * HALF_REVEAL_HEIGHT;
    assert_eq!(revealed, expected_revealed);
    assert_eq!(
        half_open_pixels
            .chunks_exact(3)
            .filter(|pixel| **pixel == MASK_COLOR)
            .count(),
        WIDTH as usize * HEIGHT as usize - expected_revealed
    );
    for row in half_open_pixels.chunks_exact(WIDTH as usize * 3) {
        for (x, pixel) in row.chunks_exact(3).enumerate() {
            if x < canvas_left || x >= canvas_left + canvas_width {
                assert_eq!(pixel, MASK_COLOR, "wipe must mask expanded side strips");
            }
        }
    }
}

#[test]
fn final_star_record_keeps_only_the_source_corner_mask() {
    let mut wipe = ScreenWipeState::inactive();
    wipe.begin(ScreenWipeKind::StarReveal);
    for _ in 1..ScreenWipeKind::StarReveal.frame_count() {
        assert!(wipe.advance());
    }
    let Some(pixels) = render(wipe) else {
        return;
    };
    let revealed = pixels
        .chunks_exact(3)
        .filter(|pixel| **pixel != MASK_COLOR)
        .count();
    assert!(revealed > WIDTH as usize * HEIGHT as usize * 3 / 4);
    assert!(revealed < WIDTH as usize * HEIGHT as usize);
}
