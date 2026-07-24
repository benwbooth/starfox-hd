use sf_core::red_fill_circle::RedFillCircleState;
use sf_render::renderer::{config_from_repo_root, FrameInputs, GameState, Renderer};

const WIDTH: i32 = 256;
const HEIGHT: i32 = 224;
const CLEAR_BLUE: u8 = 13;
const MINIMUM_RED_PIXELS: usize = 800;
const MAXIMUM_RED_PIXELS: usize = 1_300;

#[test]
fn player_death_circle_adds_red_inside_the_authored_radius() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = config_from_repo_root(&root);
    let mut renderer = match Renderer::new_headless(WIDTH, HEIGHT, &config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping red-fill render check: no wgpu adapter ({error})");
            return;
        }
    };

    let mut red_fill_circle = RedFillCircleState::inactive();
    red_fill_circle.begin();
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        red_fill_circle,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    renderer.shutdown();

    let center = ((HEIGHT as usize / 2) * WIDTH as usize + WIDTH as usize / 2) * 3;
    assert!(pixels[center] > 100);
    assert_eq!(pixels[center + 1], 0);
    assert_eq!(pixels[center + 2], CLEAR_BLUE);
    assert_eq!(&pixels[..3], &[0, 0, CLEAR_BLUE]);

    let red_pixels = pixels.chunks_exact(3).filter(|pixel| pixel[0] > 0).count();
    assert!((MINIMUM_RED_PIXELS..=MAXIMUM_RED_PIXELS).contains(&red_pixels));
}
