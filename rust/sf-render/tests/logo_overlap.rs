//! Source-authored Corneria logo decals retain their opaque backing without
//! depth flicker in the shipping smooth HD presentation.

use std::path::PathBuf;
use std::sync::Mutex;

use sf_render::draw_list::ShadowStyle;
use sf_render::draw_list::{DrawListEntry, DL_FLAG_VISIBLE};
use sf_render::renderer::{config_from_repo_root, FrameInputs, GameState, Renderer};
use sf_render::{color_data, shape_data};

const WIDTH: i32 = 320;
const HEIGHT: i32 = 224;
const MYBASE_1: u16 = sf_map::consts::sh::MYBASE_1;
const MYBASE_0: u16 = shape_data::SHAPE_EXT_MYBASE_0;
const BASE_Y: i32 = -320;
const BASE_Z: i32 = 3_000;
const BASE_YAW: i16 = 128;

static GPU_TEST_LOCK: Mutex<()> = Mutex::new(());

fn render(order: [u16; 2]) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut config = config_from_repo_root(&root);
    config.shadow_style = ShadowStyle::Smooth;
    let mut renderer = Renderer::new_headless(WIDTH, HEIGHT, &config)
        .expect("GPU required for Corneria base overlap regression");
    renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
    let entries = order.map(|shape_id| DrawListEntry {
        shape_id,
        y: BASE_Y << 16,
        z: BASE_Z << 16,
        ry: BASE_YAW,
        flags: DL_FLAG_VISIBLE,
        obj_id: shape_id,
        ..Default::default()
    });
    renderer.begin_frame();
    renderer.submit(
        &entries,
        &entries,
        1.0,
        &FrameInputs {
            game_state: GameState::Boot,
            ..Default::default()
        },
    );
    renderer.end_frame();
    let pixels = renderer.read_pixels_rgb();
    renderer.shutdown();
    pixels
}

#[test]
fn static_base_mesh_order_is_not_depth_sensitive() {
    let _guard = GPU_TEST_LOCK.lock().unwrap();
    let first = render([MYBASE_1, MYBASE_0]);
    let second = render([MYBASE_0, MYBASE_1]);
    let differing = first
        .chunks_exact(3)
        .zip(second.chunks_exact(3))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(
        differing, 0,
        "Corneria base image changes for the two static mesh orders ({differing} pixels)"
    );
}

/// Compare actual authored face pairs with independently rendered layers.
/// Opaque logo pixels must win, and transparent pixels must retain the wall.
#[test]
fn authored_logo_pairs_preserve_both_decal_and_backing_across_camera_poses() {
    const HD_WIDTH: i32 = 1280;
    const HD_HEIGHT: i32 = 720;
    const FRACTIONAL_CAMERA_ROTATION: [u16; 3] = [173, 251, 419];
    const LOGO_CASES: &[(u16, &[usize], i32)] = &[
        (shape_data::SHAPE_EXT_MYBASE_0, &[48, 50], 100),
        (sf_map::consts::sh::BU_7, &[7], 350),
    ];
    const DEPTHS: [i32; 3] = [800, 1_200, 2_000];
    const YAWS: [i16; 5] = [-13, -5, 0, 7, 19];
    const MINIMUM_COVERAGE: usize = 5;
    let _guard = GPU_TEST_LOCK.lock().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut config = config_from_repo_root(&root);
    config.shadow_style = ShadowStyle::Smooth;
    let mut renderer = Renderer::new_headless(HD_WIDTH, HD_HEIGHT, &config).unwrap();
    for &(shape_id, backing_faces, center_y) in LOGO_CASES {
        let asset = shape_data::SHAPE_DATA
            .iter()
            .find(|asset| asset.shape_id == shape_id)
            .unwrap();
        for &face_index in backing_faces {
            let mut faces = [asset.faces[face_index], asset.faces[face_index + 1]];
            // This fixture isolates depth composition, independently of visibility.
            for face in &mut faces {
                face.visibility_vertices = None;
            }
            for depth in DEPTHS {
                for yaw in YAWS {
                    let entry = DrawListEntry {
                        shape_id,
                        y: center_y << 16,
                        z: depth << 16,
                        ry: yaw,
                        flags: DL_FLAG_VISIBLE,
                        obj_id: 1,
                        ..Default::default()
                    };
                    let mut capture = |selected: &[shape_data::ShapeFace]| {
                        assert!(renderer.shapes.register_with_color(
                            entry.shape_id,
                            asset.vertices,
                            selected,
                            color_data::COLOR_TABLE_ID_0_C
                        ));
                        renderer
                            .transform
                            .set_camera_fine(0, 0, 0, FRACTIONAL_CAMERA_ROTATION);
                        renderer.begin_frame();
                        renderer.submit(&[entry], &[entry], 1.0, &FrameInputs::default());
                        renderer.end_frame();
                        renderer.read_pixels_rgb()
                    };
                    let backing = capture(&faces[..1]);
                    let decal = capture(&faces[1..]);
                    let combined = capture(&faces);
                    let mut opaque_count = 0;
                    let mut transparent_count = 0;
                    let mut mismatches = 0;
                    for ((wall, logo), actual) in backing
                        .chunks_exact(3)
                        .zip(decal.chunks_exact(3))
                        .zip(combined.chunks_exact(3))
                    {
                        let expected = if logo != [0, 0, 0] {
                            opaque_count += 1;
                            logo
                        } else {
                            if wall != [0, 0, 0] {
                                transparent_count += 1;
                            }
                            wall
                        };
                        if actual != expected {
                            mismatches += 1;
                        }
                    }
                    assert!(opaque_count >= MINIMUM_COVERAGE && transparent_count >= MINIMUM_COVERAGE,
                    "face {face_index}, depth {depth}, yaw {yaw}: missing coverage ({opaque_count}, {transparent_count})");
                    assert_eq!(mismatches, 0,
                    "shape {shape_id}, face {face_index}, depth {depth}, yaw {yaw}: logo/wall composition differs");
                }
            }
        }
    }
    renderer.shutdown();
}
