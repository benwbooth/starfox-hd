//! Compare plane construction and vertex distances against unmodified retail
//! Super FX routines. Machine setup lives only in this verification harness.

use sf2_data::shape_data::{ShapeVertex, SHAPE_DATA};
use sf_oracle::gsu::Gsu;
use sf_render::sf2_clipping::{ClipPlane, PlaneTransform};

const PLANE_ENTRY: u16 = 0xF0AD;
const PLANE_EXIT: u16 = 0xF1BB;
const DISTANCE_ENTRY: u16 = 0xF2FA;
const DISTANCE_EXIT: u16 = 0xF372;
const MATRIX_FIELDS: [[usize; 3]; 3] = [
    [0x132, 0x138, 0x13E],
    [0x134, 0x13A, 0x140],
    [0x136, 0x13C, 0x142],
];
const TRANSLATION_FIELDS: [usize; 3] = [0x26, 0x28, 0x2A];
const PLANE_TABLE: usize = 0x2818;
const PLANE_SIZE: usize = 8;
const SETUP_ADDRESS: usize = 0x6000;
const WORD_BYTES: usize = 2;
const STEP_LIMIT: usize = 4096;

fn retail() -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("clipping differential tests require the user-owned retail SF2 ROM")
}

fn components(vector: ShapeVertex) -> [i16; 3] {
    [vector.x, vector.y, vector.z]
}

fn write_word(exact: &mut Gsu, address: usize, value: i16) {
    exact.ram[address..address + WORD_BYTES].copy_from_slice(&value.to_le_bytes());
}

fn read_word(exact: &Gsu, address: usize) -> i16 {
    i16::from_le_bytes(exact.ram[address..address + WORD_BYTES].try_into().unwrap())
}

fn run_to(exact: &mut Gsu, entry: u16, exit: u16) {
    exact.start(1, entry);
    for _ in 0..STEP_LIMIT {
        if exact.execution_state().1 == exit {
            return;
        }
        assert!(!exact.run_slice(1), "retail stopped before expected exit");
    }
    panic!("retail clipping did not reach {exit:04X}");
}

fn oracle(rom: &[u8]) -> Gsu {
    let mut exact = Gsu::new(rom.to_vec());
    // Harness-only ROMB setup; the retail code and authored records are not
    // patched. Both clipping shapes read from ROM bank $10.
    let setup = [0xA0, 0x10, 0x3F, 0xDF, 0x00];
    exact.ram[SETUP_ADDRESS..SETUP_ADDRESS + setup.len()].copy_from_slice(&setup);
    exact.run_with_limit(0x60, SETUP_ADDRESS as u16, 16);
    assert!(!exact.last_run_hit_limit);
    exact
}

#[test]
fn authored_planes_and_vertex_distances_match_retail() {
    const ORIGIN: ShapeVertex = ShapeVertex { x: 0, y: 0, z: 0 };
    const TRANSLATIONS: [ShapeVertex; 4] = [
        ORIGIN,
        ShapeVertex {
            x: -750,
            y: 53,
            z: 1500,
        },
        ShapeVertex {
            x: i16::MIN,
            y: i16::MAX,
            z: -1,
        },
        ShapeVertex {
            x: 2791,
            y: -1137,
            z: -801,
        },
    ];
    const VERTICES: [ShapeVertex; 7] = [
        ORIGIN,
        ShapeVertex { x: 1, y: -1, z: 1 },
        ShapeVertex { x: -1, y: 1, z: -1 },
        ShapeVertex {
            x: 317,
            y: -137,
            z: 509,
        },
        ShapeVertex {
            x: -2791,
            y: 1137,
            z: 801,
        },
        ShapeVertex {
            x: i16::MIN,
            y: i16::MAX,
            z: -1,
        },
        ShapeVertex {
            x: i16::MAX,
            y: i16::MIN,
            z: 1,
        },
    ];
    let rom = retail();
    assert_eq!(&rom[0x8068..0x806C], &[0xFF, 0xAD, 0xF0, 0x01]);
    for angle in 0..=u8::MAX {
        // Cover all 256 orientations on each axis with combined transforms.
        let matrix = sf_core::snes_trig::zxy_matrix_q15(
            angle,
            angle.wrapping_mul(3),
            angle.wrapping_add(80),
        );
        let row = |values: [i16; 3]| ShapeVertex {
            x: values[0],
            y: values[1],
            z: values[2],
        };
        let transform = PlaneTransform {
            x: row(matrix[0]),
            y: row(matrix[1]),
            z: row(matrix[2]),
        };
        for shape_index in [48, 49] {
            let shape = &SHAPE_DATA[shape_index];
            for (plane_index, definition) in shape.clipping_planes.iter().enumerate() {
                let mut exact = oracle(&rom);
                for (fields, values) in MATRIX_FIELDS.into_iter().zip(matrix) {
                    for (field, value) in fields.into_iter().zip(values) {
                        write_word(&mut exact, field, value);
                    }
                }
                for translation in TRANSLATIONS {
                    for (field, value) in
                        TRANSLATION_FIELDS.into_iter().zip(components(translation))
                    {
                        write_word(&mut exact, field, value);
                    }
                    exact.r[14] = shape.faces_address as u16 + (plane_index as u16 * 14) + 1;
                    run_to(&mut exact, PLANE_ENTRY, PLANE_EXIT);
                    let native = ClipPlane::from_definition(*definition, transform, translation);
                    let plane_address =
                        PLANE_TABLE + (usize::from(definition.slot) - 1) * PLANE_SIZE;
                    for (index, expected) in components(native.normal)
                        .into_iter()
                        .chain([native.distance])
                        .enumerate()
                    {
                        assert_eq!(read_word(&exact, plane_address + index * WORD_BYTES), expected,
                            "angle={angle} shape={shape_index} plane={plane_index} translation={translation:?} field={index}");
                    }

                    let mesh_translation = ShapeVertex {
                        x: -101,
                        y: 153,
                        z: 771,
                    };
                    for (field, value) in TRANSLATION_FIELDS
                        .into_iter()
                        .zip(components(mesh_translation))
                    {
                        write_word(&mut exact, field, value);
                    }
                    for (index, vertex) in VERTICES.into_iter().enumerate() {
                        for (axis, value) in components(vertex).into_iter().enumerate() {
                            write_word(&mut exact, 0x04F0 + (index * 3 + axis) * WORD_BYTES, value);
                        }
                    }
                    write_word(&mut exact, 0x144, VERTICES.len() as i16);
                    exact.r[1] = u16::from(definition.slot);
                    run_to(&mut exact, DISTANCE_ENTRY, DISTANCE_EXIT);
                    let relative = native.relative_to(mesh_translation);
                    for (index, vertex) in VERTICES.into_iter().enumerate() {
                        assert_eq!(read_word(&exact, 0x2778 + index * WORD_BYTES), relative.signed_distance(vertex),
                            "angle={angle} shape={shape_index} plane={plane_index} vertex={vertex:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn mesen_plane_capture_matches_native() {
    const CAPTURE: &str = include_str!("../../../tools/sf2/fixtures/logo_clipping_planes.csv");
    const FIELD_COUNT: usize = 18;
    const EXPECTED_CAPTURE_ROWS: usize = 236;
    const HEADER: &str = "frame,slot,xx,xy,xz,yx,yy,yz,zx,zy,zz,x,y,z,nx,ny,nz,distance";
    let latest = std::env::var_os("SF2_CLIPPING_TRACE").map(|path| {
        std::fs::read_to_string(path)
            .expect("SF2_CLIPPING_TRACE must name a readable Mesen capture")
    });
    for capture in std::iter::once(CAPTURE).chain(latest.as_deref()) {
        let mut lines = capture.lines();
        assert_eq!(lines.next(), Some(HEADER));
        let mut count = 0;
        let mut seen = [false; 2];
        for line in lines {
            let fields: Vec<i32> = line
                .split(',')
                .map(|field| field.parse().unwrap())
                .collect();
            assert_eq!(fields.len(), FIELD_COUNT);
            let frame = fields[0];
            let slot = fields[1];
            let index = match slot {
                4 => 0,
                5 => 1,
                _ => panic!("unexpected captured logo clipping slot {slot}"),
            };
            seen[index] = true;
            let vector = |start: usize| ShapeVertex {
                x: i16::try_from(fields[start]).unwrap(),
                y: i16::try_from(fields[start + 1]).unwrap(),
                z: i16::try_from(fields[start + 2]).unwrap(),
            };
            let transform = PlaneTransform {
                x: vector(2),
                y: vector(5),
                z: vector(8),
            };
            let translation = vector(11);
            let native = ClipPlane::from_definition(
                SHAPE_DATA[48].clipping_planes[index],
                transform,
                translation,
            );
            assert_eq!(native.normal, vector(14), "Mesen frame={frame} slot={slot}");
            assert_eq!(
                i32::from(native.distance),
                fields[17],
                "Mesen frame={frame} slot={slot}"
            );
            count += 1;
        }
        assert_eq!(seen, [true; 2]);
        assert_eq!(
            count, EXPECTED_CAPTURE_ROWS,
            "capture must be a complete default 800-frame run"
        );
    }
}
