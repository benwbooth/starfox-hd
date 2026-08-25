use sf_core::screen_fill_circle::{ScreenFillCircleCenter, ScreenFillCircleState};
use sf_render::renderer::{config_from_repo_root, FrameInputs, GameState, Renderer};

const WIDTH: i32 = 256;
const HEIGHT: i32 = 224;
const CLEAR_COMPONENT: u8 = 0;
const MINIMUM_RED_PIXELS: usize = 800;
const MAXIMUM_RED_PIXELS: usize = 1_300;
const OBJECT_WORLD_X: i32 = 40;
const OBJECT_WORLD_Z: i32 = 500;
const OBJECT_SCREEN_X: usize = 148;

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

    let mut screen_fill_circle = ScreenFillCircleState::inactive();
    screen_fill_circle.begin_red(ScreenFillCircleCenter::Screen);
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        screen_fill_circle,
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
    assert_eq!(pixels[center + 2], CLEAR_COMPONENT);
    assert_eq!(&pixels[..3], &[CLEAR_COMPONENT; 3]);

    let red_pixels = pixels.chunks_exact(3).filter(|pixel| pixel[0] > 0).count();
    assert!((MINIMUM_RED_PIXELS..=MAXIMUM_RED_PIXELS).contains(&red_pixels));
}

#[test]
fn white_fill_projects_its_typed_object_center() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = config_from_repo_root(&root);
    let mut renderer = match Renderer::new_headless(WIDTH, HEIGHT, &config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping white-fill render check: no wgpu adapter ({error})");
            return;
        }
    };
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);

    let mut screen_fill_circle = ScreenFillCircleState::inactive();
    screen_fill_circle.begin_white(ScreenFillCircleCenter::World {
        x: OBJECT_WORLD_X as i16,
        y: 0,
        z: OBJECT_WORLD_Z as i16,
    });
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        screen_fill_circle,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&[], &[], 1.0, &inputs);
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    renderer.shutdown();

    let anchored = ((HEIGHT as usize / 2) * WIDTH as usize + OBJECT_SCREEN_X) * 3;
    assert!(pixels[anchored] > 100);
    assert!(pixels[anchored + 1] > 100);
    assert!(pixels[anchored + 2] > 100);

    let source_center = ((HEIGHT as usize / 2) * WIDTH as usize + WIDTH as usize / 2) * 3;
    assert_eq!(
        &pixels[source_center..source_center + 3],
        &[CLEAR_COMPONENT; 3]
    );
}
