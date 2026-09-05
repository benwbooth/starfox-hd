//! Resize the real renderer: 2D pixels keep their scale and placement while
//! HD flight can draw world objects outside the centered interface canvas.
use sf_render::draw_list::{DrawListEntry, ShadowStyle, DL_FLAG_VISIBLE};
use sf_render::renderer::{config_from_repo_root, FrameInputs, GameState, Renderer};
use sf_render::shapes::SHAPE_MYSHIP_4;

const CANVAS_WIDTH: i32 = 512;
const CANVAS_HEIGHT: i32 = 448;
const OUTPUTS: [(i32, i32); 3] = [(1024, 448), (512, 896), (512, 448)];
const STATES: [GameState; 6] = [
    GameState::Title,
    GameState::PlanetSelect,
    GameState::Briefing,
    GameState::Continue,
    GameState::AttractIntro,
    GameState::Playing,
];
const MINIMUM_ART_PIXELS: usize = 100;
const OUTSIDE_WORLD_X: i32 = 800;
const WORLD_DEPTH: i32 = 1000;

fn capture(renderer: &mut Renderer, state: GameState, entries: &[DrawListEntry]) -> Vec<u8> {
    capture_with_meters(renderer, state, entries, 1)
}

fn capture_with_meters(
    renderer: &mut Renderer,
    state: GameState,
    entries: &[DrawListEntry],
    meters: u16,
) -> Vec<u8> {
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    renderer.begin_frame();
    renderer.submit(
        entries,
        entries,
        1.0,
        &FrameInputs {
            game_state: state,
            currentbg: if matches!(state, GameState::Playing | GameState::AttractIntro) {
                sf_map::catalog::background_id::ONE_ONE_OUTDOOR
            } else {
                0
            },
            meters,
            ..Default::default()
        },
    );
    renderer.end_frame();
    renderer.read_pixels_rgb()
}

#[test]
fn resizing_preserves_screen_art_and_extends_only_the_flight_world() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut config = config_from_repo_root(&root);
    config.shadow_style = ShadowStyle::Smooth;
    let mut renderer = Renderer::new_headless(CANVAS_WIDTH, CANVAS_HEIGHT, &config)
        .expect("GPU required for resize regression");
    for state in STATES {
        renderer.resize(CANVAS_WIDTH, CANVAS_HEIGHT);
        let canonical = capture(&mut renderer, state, &[]);
        let without_hud = capture_with_meters(&mut renderer, state, &[], 0);
        assert!(
            canonical
                .chunks_exact(3)
                .filter(|pixel| *pixel != [0; 3])
                .count()
                > MINIMUM_ART_PIXELS,
            "empty {state:?} fixture"
        );
        for (width, height) in OUTPUTS {
            renderer.resize(width, height);
            let actual = capture(&mut renderer, state, &[]);
            let left = (width - CANVAS_WIDTH) / 2;
            let top = (height - CANVAS_HEIGHT) / 2;
            if state != GameState::Playing || height == CANVAS_HEIGHT {
                for y in 0..CANVAS_HEIGHT {
                    let expected_row = (y * CANVAS_WIDTH * 3) as usize;
                    let actual_row = ((y + top) * width * 3 + left * 3) as usize;
                    let row_bytes = (CANVAS_WIDTH * 3) as usize;
                    assert_eq!(
                        &actual[actual_row..actual_row + row_bytes],
                        &canonical[expected_row..expected_row + row_bytes],
                        "{state:?}, {width}x{height}: 2D canvas changed at row {y}",
                    );
                }
            }
            if state == GameState::Playing {
                let mut checked = 0;
                for (index, (hud, background)) in canonical
                    .chunks_exact(3)
                    .zip(without_hud.chunks_exact(3))
                    .enumerate()
                {
                    if hud != background {
                        let x = index as i32 % CANVAS_WIDTH + left;
                        let y = index as i32 / CANVAS_WIDTH + top;
                        let offset = ((y * width + x) * 3) as usize;
                        assert_eq!(
                            &actual[offset..offset + 3],
                            hud,
                            "HUD stretched at {width}x{height}"
                        );
                        checked += 1;
                    }
                }
                assert!(checked > MINIMUM_ART_PIXELS);
            }
            if state != GameState::Playing {
                for y in 0..height {
                    for x in 0..width {
                        if x < left
                            || x >= left + CANVAS_WIDTH
                            || y < top
                            || y >= top + CANVAS_HEIGHT
                        {
                            let offset = ((y * width + x) * 3) as usize;
                            assert_eq!(
                                &actual[offset..offset + 3],
                                &[0; 3],
                                "art leaked into bars"
                            );
                        }
                    }
                }
            }
        }
    }
    renderer.resize(OUTPUTS[0].0, OUTPUTS[0].1);
    let empty = capture(&mut renderer, GameState::Playing, &[]);
    let ship = [DrawListEntry {
        shape_id: SHAPE_MYSHIP_4,
        x: OUTSIDE_WORLD_X << 16,
        z: WORLD_DEPTH << 16,
        flags: DL_FLAG_VISIBLE,
        obj_id: 1,
        ..Default::default()
    }];
    let world = capture(&mut renderer, GameState::Playing, &ship);
    let right_edge = (OUTPUTS[0].0 + CANVAS_WIDTH) / 2;
    let extended_pixels = world
        .chunks_exact(3)
        .zip(empty.chunks_exact(3))
        .enumerate()
        .filter(|(index, (a, b))| (*index as i32 % OUTPUTS[0].0) >= right_edge && a != b)
        .count();
    assert!(
        extended_pixels > MINIMUM_ART_PIXELS,
        "HD world was clipped to the 2D canvas"
    );
    // Minimize/restore events must not leave mismatched attachments or a
    // zero-sized projection behind.
    renderer.resize(0, 0);
    assert_eq!(capture(&mut renderer, GameState::Boot, &[]).len(), 3);
    renderer.resize(CANVAS_WIDTH, CANVAS_HEIGHT);
    assert_eq!(
        capture(&mut renderer, GameState::Title, &[]).len(),
        (CANVAS_WIDTH * CANVAS_HEIGHT * 3) as usize
    );
    renderer.shutdown();
}
