//! Render the actual player mesh as a reference for application artwork.
use sf_render::draw_list::{DrawListEntry, ShadowStyle, DL_FLAG_VISIBLE};
use sf_render::renderer::{config_from_repo_root, FrameInputs, GameState, Renderer};
use sf_render::shapes::SHAPE_MYSHIP_4;
use std::io::Write;

const WIDTH: i32 = 1024;
const HEIGHT: i32 = 896;
const DEPTH: i32 = 500;
const PITCH: i16 = 24;
const FRONT_YAW: i16 = 128;
const WORLD_FRACTION_BITS: u32 = 16;
const MINIMUM_VISIBLE_PIXELS: usize = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).expect("output PPM path required");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut config = config_from_repo_root(&root);
    config.shadow_style = ShadowStyle::Smooth;
    let mut renderer = Renderer::new_headless(WIDTH, HEIGHT, &config)?;
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    let arwing = DrawListEntry {
        shape_id: SHAPE_MYSHIP_4,
        z: DEPTH << WORLD_FRACTION_BITS,
        rx: PITCH,
        ry: FRONT_YAW,
        flags: DL_FLAG_VISIBLE,
        obj_id: 1,
        ..Default::default()
    };
    renderer.begin_frame();
    renderer.submit(
        &[arwing],
        &[arwing],
        1.0,
        &FrameInputs {
            game_state: GameState::Boot,
            ..Default::default()
        },
    );
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    assert!(
        pixels
            .chunks_exact(3)
            .filter(|pixel| *pixel != [0; 3])
            .count()
            > MINIMUM_VISIBLE_PIXELS,
        "reference must contain a visible player mesh"
    );
    let mut file = std::io::BufWriter::new(std::fs::File::create(output)?);
    writeln!(file, "P6\n{WIDTH} {HEIGHT}\n255")?;
    file.write_all(&pixels)?;
    renderer.shutdown();
    Ok(())
}
