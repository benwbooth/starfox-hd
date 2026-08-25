use sf_render::draw_list::{DrawListEntry, DL_FLAG_SCALED_SPRITE, DL_FLAG_VISIBLE};
use sf_render::renderer::{config_from_repo_root, FrameInputs, GameState, Renderer};

const WIDTH: i32 = 256;
const HEIGHT: i32 = 224;
const TEST_DEPTH: i32 = 300;
const MEDIUM_EXPLOSION_SPRITE_SHAPE: u16 = 462;
const PLAYER_SPRITE_SCALE_ADJUSTMENT: u8 = 253;
const CLEAR_COLOR: [u8; 3] = [0, 0, 0];
const MINIMUM_ADJUSTED_PIXELS: usize = 50;
const MINIMUM_AREA_RATIO: usize = 2;

fn rendered_pixels(renderer: &mut Renderer, adjustment: u8) -> usize {
    let entry = DrawListEntry {
        z: TEST_DEPTH << 16,
        shape_id: MEDIUM_EXPLOSION_SPRITE_SHAPE,
        flags: DL_FLAG_VISIBLE | DL_FLAG_SCALED_SPRITE,
        tscroll_x: adjustment,
        obj_id: 1,
        ..Default::default()
    };
    let inputs = FrameInputs {
        game_state: GameState::Boot,
        source_resolution: true,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(&[entry], &[entry], 1.0, &inputs);
    renderer.end_frame();
    renderer
        .read_pixels_rgb()
        .chunks_exact(3)
        .filter(|pixel| *pixel != CLEAR_COLOR)
        .count()
}

#[test]
fn player_sized_explosion_uses_the_source_signed_sprite_adjustment() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = config_from_repo_root(&root);
    let mut renderer = match Renderer::new_headless(WIDTH, HEIGHT, &config) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skipping explosion-sprite render check: no wgpu adapter ({error})");
            return;
        }
    };
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);

    let base_pixels = rendered_pixels(&mut renderer, 0);
    let adjusted_pixels = rendered_pixels(&mut renderer, PLAYER_SPRITE_SCALE_ADJUSTMENT);
    renderer.shutdown();

    assert!(
        adjusted_pixels >= MINIMUM_ADJUSTED_PIXELS,
        "base={base_pixels} adjusted={adjusted_pixels}"
    );
    assert!(
        base_pixels >= adjusted_pixels * MINIMUM_AREA_RATIO,
        "base={base_pixels} adjusted={adjusted_pixels}"
    );
}
