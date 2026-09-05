//! HD billboards must show one upright source bitmap, even when their signed
//! size byte would wrap ordinary polygon texture coordinates.
use sf_render::draw_list::{DrawListEntry, DL_FLAG_SCALED_SPRITE, DL_FLAG_VISIBLE};
use sf_render::renderer::{config_from_repo_root, FrameInputs, Renderer};
use sf_render::{color_data, shape_data, shapes};

const WIDTH: i32 = 1280;
const HEIGHT: i32 = 720;
const DEPTH: i32 = 300;
const BITMAP_SIDE: usize = 32;
const TEXTURE_ROW_BYTES: usize = 256;
const TEXTURE_BANK_MASK: usize = 32767;
const DESCRIPTOR_MASK: u16 = 255;
const HIGH_NIBBLE_FLAG: u16 = 0x2000;
const RGB_TOLERANCE: i32 = 1;
const TEXEL_EDGE_EPSILON: f32 = 0.001;
const MINIMUM_CHECKED_PIXELS: usize = 5000;
const BOOST_SIZES: [(u8, f32); 3] = [(255, 38.0), (251, 30.0), (0, 40.0)];

#[test]
fn hd_launch_boost_matches_one_complete_upright_bitmap_at_each_size() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = config_from_repo_root(&root);
    let mut renderer = Renderer::new_headless(WIDTH, HEIGHT, &config)
        .expect("GPU required for billboard bitmap regression");
    let texture_banks = [
        std::fs::read(root.join("reference/ultrastarfox/SF/MSPRITES/TEX_01.BIN")).unwrap(),
        std::fs::read(root.join("reference/ultrastarfox/SF/MSPRITES/TEX_23.BIN")).unwrap(),
    ];
    let inputs = FrameInputs::default();
    let palette =
        shapes::decode_shape_palette(shapes::game_palette_bgr(inputs.scene_style.game_palette));
    for color_frame in [0, 1] {
        let material = shapes::resolve_face_material_from_table(
            color_data::COLOR_TABLE_BOOST_C,
            0,
            color_frame,
            0,
        )
        .unwrap();
        let descriptor = &color_data::TEXTURE_SPRITES[usize::from(material & DESCRIPTOR_MASK)];
        let texture = &texture_banks[usize::from(descriptor.bank)];
        for (adjustment, world_width) in BOOST_SIZES {
            let draw = [DrawListEntry {
                shape_id: shape_data::SHAPE_EXT_BOOSTSHAPE,
                z: DEPTH << 16,
                flags: DL_FLAG_VISIBLE | DL_FLAG_SCALED_SPRITE,
                tscroll_x: adjustment,
                col_frame: color_frame,
                obj_id: 1,
                ..Default::default()
            }];
            renderer.transform.set_camera(0, 0, 0, 0, 0, 0);
            renderer.begin_frame();
            renderer.submit(&draw, &draw, 1.0, &inputs);
            renderer.end_frame();
            let pixels = renderer.read_pixels_rgb();
            let projection = renderer.transform.projection();
            let width = projection[0] * world_width / DEPTH as f32 * WIDTH as f32 / 2.0;
            let height = projection[5] * world_width / DEPTH as f32 * HEIGHT as f32 / 2.0;
            assert!(
                (width - height).abs() < TEXEL_EDGE_EPSILON,
                "sprite must remain square"
            );
            let left = (WIDTH as f32 - width) / 2.0;
            let top = (HEIGHT as f32 - height) / 2.0;
            let mut checked = 0;
            let mut mismatches = 0;
            for y in top.ceil() as usize..(top + height).floor() as usize {
                for x in left.ceil() as usize..(left + width).floor() as usize {
                    let tx = (x as f32 + 0.5 - left) / width * BITMAP_SIDE as f32;
                    let ty = (y as f32 + 0.5 - top) / height * BITMAP_SIDE as f32;
                    // Avoid implementation-dependent floating-point rounding
                    // exactly on a nearest-neighbor texel boundary.
                    if (tx - tx.round()).abs() < TEXEL_EDGE_EPSILON
                        || (ty - ty.round()).abs() < TEXEL_EDGE_EPSILON
                    {
                        continue;
                    }
                    let address = (usize::from(descriptor.offset)
                        + ty.floor() as usize * TEXTURE_ROW_BYTES
                        + tx.floor() as usize)
                        & TEXTURE_BANK_MASK;
                    let packed = texture[address];
                    let index = if material & HIGH_NIBBLE_FLAG != 0 {
                        packed >> 4
                    } else {
                        packed & 15
                    };
                    let expected = if index == 0 {
                        [0; 3]
                    } else {
                        palette[usize::from(index)].map(|channel| (channel * 255.0).round() as u8)
                    };
                    let offset = (y * WIDTH as usize + x) * 3;
                    if (0..3).any(|channel| {
                        (i32::from(pixels[offset + channel]) - i32::from(expected[channel])).abs()
                            > RGB_TOLERANCE
                    }) {
                        mismatches += 1;
                    }
                    checked += 1;
                }
            }
            assert!(checked > MINIMUM_CHECKED_PIXELS);
            assert_eq!(mismatches, 0,
                "frame {color_frame}, size byte {adjustment}: {mismatches}/{checked} pixels differ from one source bitmap");
        }
    }
}
