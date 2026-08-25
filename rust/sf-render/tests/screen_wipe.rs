use sf_core::screen_wipe::{ScreenWipeKind, ScreenWipeState};
use sf_render::renderer::{config_from_repo_root, FrameInputs, GameState, Renderer};

const WIDTH: i32 = 224;
const HEIGHT: i32 = 192;
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
fn horizontal_reveal_masks_the_actual_output_surface() {
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
    assert_eq!(revealed, WIDTH as usize * 96);
    assert_eq!(
        half_open_pixels
            .chunks_exact(3)
            .filter(|pixel| **pixel == MASK_COLOR)
            .count(),
        WIDTH as usize * 96
    );
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
