use sf2_data::{
    point_program::PointFormat,
    point_program_data::POINT_PROGRAMS,
    shape_data::{ShapeDataEntry, SHAPE_DATA},
};
use sf2_game::intro_transform::transform_shape;
use sf_oracle::gsu::Gsu;

fn rom() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM")
}

fn put_word(source: &mut Gsu, address: usize, value: u16) {
    source.ram[address..address + 2].copy_from_slice(&value.to_le_bytes());
}

fn word(source: &Gsu, address: usize) -> u16 {
    u16::from_le_bytes([source.ram[address], source.ram[address + 1]])
}

fn prepare(source: &mut Gsu, shape: &ShapeDataEntry, animation: u16, columns: [[i16; 3]; 3]) {
    for (index, coefficient) in columns.into_iter().flatten().enumerate() {
        put_word(source, 0x132 + index * 2, coefficient as u16);
    }
    put_word(source, 0x30, 1u16 << shape.shift);
    put_word(source, 0x32, u16::from(shape.shift));
    put_word(source, 0x44, animation);
    put_word(source, 0x16, shape.points_address as u16);
    put_word(source, 0x1C, (shape.points_address >> 16) as u16);
    source.r[10] = 0x400;
}

const MATRICES: [[[i16; 3]; 3]; 4] = [
    [[32767, 0, 0], [0, 32767, 0], [0, 0, 32767]],
    [[0, -32768, 0], [32767, 0, 0], [0, 0, 32767]],
    [
        [23171, -10927, 8931],
        [1787, 29303, -20211],
        [-14377, 61, 26777],
    ],
    [
        [-32768, 32767, -1],
        [32767, -32768, 255],
        [-32768, 1, 32767],
    ],
];

#[test]
fn point_block_catalog_preserves_every_source_record() {
    let rom = rom();
    assert_eq!(SHAPE_DATA.len(), POINT_PROGRAMS.len());
    let mut inputs = 0;
    for (shape, program) in SHAPE_DATA.iter().zip(&POINT_PROGRAMS) {
        assert_eq!(program.frames.len(), shape.animation_frames.len().max(1));
        for (frame, blocks) in program.frames.iter().enumerate() {
            let vertices = if shape.animation_frames.is_empty() {
                shape.vertices
            } else {
                shape.animation_frames[frame]
            };
            let mut cursor = 0;
            for block in *blocks {
                let bank = (block.source_address >> 16) as usize;
                let mut address = bank * 0x8000 + (block.source_address as usize & 0x7FFF);
                let opcode = match (block.format, block.mirrored) {
                    (PointFormat::Bytes, false) => 4,
                    (PointFormat::Words, false) => 8,
                    (PointFormat::Words, true) => 0x34,
                    (PointFormat::Bytes, true) => 0x38,
                };
                assert_eq!(rom[address], opcode);
                assert_eq!(rom[address + 1], block.count);
                assert_ne!(block.count, 0);
                assert_eq!(cursor, usize::from(block.first_vertex));
                address += 2;
                for _ in 0..block.count {
                    let point: [i16; 3] = std::array::from_fn(|_| {
                        if block.format == PointFormat::Words {
                            let value = i16::from_le_bytes([rom[address], rom[address + 1]]);
                            address += 2;
                            value
                        } else {
                            let value = i16::from(rom[address] as i8);
                            address += 1;
                            value
                        }
                    });
                    let vertex = vertices[cursor];
                    assert_eq!([vertex.x, vertex.y, vertex.z], point);
                    cursor += 1;
                    if block.mirrored {
                        let vertex = vertices[cursor];
                        assert_eq!(
                            [vertex.x, vertex.y, vertex.z],
                            [point[0].wrapping_neg(), point[1], point[2]]
                        );
                        cursor += 1;
                    }
                    inputs += 1;
                }
            }
            assert_eq!(cursor, vertices.len());
        }
    }
    assert_eq!(inputs, 28032);
}

#[test]
fn native_rotation_matches_original_for_every_shape_frame() {
    let mut source = Gsu::new(rom());
    assert_eq!(SHAPE_DATA.len(), POINT_PROGRAMS.len());
    let mut comparisons = 0;
    for (shape, program) in SHAPE_DATA.iter().zip(&POINT_PROGRAMS) {
        if shape.points_address == 0 {
            assert!(shape.vertices.is_empty());
            continue;
        }
        for columns in MATRICES {
            for animation in (0..program.frames.len() as u16).chain([64, 65, 127, u16::MAX]) {
                let native = transform_shape(shape, program, animation, columns).unwrap();
                prepare(&mut source, shape, animation, columns);
                // Begin at the original high-byte matrix packing/list setup,
                // not at an individual point handler selected by the native side.
                source.watch_execution(1, 0x9A84);
                source.start(1, 0x96BA);
                while !source.execution_watch_hit()
                    && source.is_running()
                    && source.last_run_steps < 100_000
                {
                    source.run_slice(1);
                }
                assert!(
                    source.execution_watch_hit(),
                    "shape={} animation={animation}",
                    shape.header_index
                );
                assert_eq!(usize::from(word(&source, 0x144)), native.len());
                assert_eq!(usize::from(word(&source, 0x1E)), 0x4F0 + native.len() * 6);
                for (index, point) in native.iter().enumerate() {
                    let actual: [i16; 3] = std::array::from_fn(|axis| {
                        word(&source, 0x4F0 + index * 6 + axis * 2) as i16
                    });
                    assert_eq!(
                        *point, actual,
                        "shape={} animation={animation} point={index} matrix={columns:?}",
                        shape.header_index
                    );
                }
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 15428);
}

#[test]
fn all_point_formats_match_original_at_coordinate_and_scale_extremes() {
    use sf2_data::{point_program::PointBlock, shape_data::ShapeVertex};
    use sf2_game::intro_transform::{transform_points, PointTransform};
    let mut fixture = rom();
    let mut stream = Vec::new();
    let mut vertices = Vec::new();
    let mut blocks = Vec::new();
    for (opcode, format, mirrored) in [
        (4, PointFormat::Bytes, false),
        (8, PointFormat::Words, false),
        (0x34, PointFormat::Words, true),
        (0x38, PointFormat::Bytes, true),
    ] {
        let points = if format == PointFormat::Bytes {
            [
                [-128, 127, -1],
                [127, -128, 1],
                [1, 1, 1],
                [-1, -1, -1],
                [0, 127, -128],
                [68, -109, -31],
            ]
        } else {
            [
                [-32768, 32767, -1],
                [32767, -32768, 1],
                [1, 1, 1],
                [-1, -1, -1],
                [0, 32767, -32768],
                [16385, -10927, -313],
            ]
        };
        blocks.push(PointBlock {
            source_address: 0xF000 + stream.len() as u32,
            format,
            mirrored,
            first_vertex: vertices.len() as u16,
            count: points.len() as u8,
        });
        stream.extend([opcode, points.len() as u8]);
        for [x, y, z] in points {
            vertices.push(ShapeVertex { x, y, z });
            if mirrored {
                vertices.push(ShapeVertex {
                    x: x.wrapping_neg(),
                    y,
                    z,
                });
            }
            for coordinate in [x, y, z] {
                if format == PointFormat::Words {
                    stream.extend(coordinate.to_le_bytes());
                } else {
                    stream.push(coordinate as u8);
                }
            }
        }
    }
    stream.push(0x0C);
    fixture[0x7000..0x7000 + stream.len()].copy_from_slice(&stream);
    let mut source = Gsu::new(fixture);
    let mut random = 0xC4AA_90E1u32;
    for case in 0..1024 {
        let columns = std::array::from_fn(|_| {
            std::array::from_fn(|_| {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                random as i16
            })
        });
        let shift = (case % 16) as u8;
        let transform = PointTransform::new(columns, shift).unwrap();
        let native = transform_points(&vertices, &blocks, transform).unwrap();
        let shape = ShapeDataEntry {
            points_address: 0xF000,
            shift,
            ..SHAPE_DATA[0]
        };
        prepare(&mut source, &shape, 0, columns);
        source.watch_execution(1, 0x9A84);
        source.start(1, 0x96BA);
        while !source.execution_watch_hit()
            && source.is_running()
            && source.last_run_steps < 100_000
        {
            source.run_slice(1);
        }
        assert!(source.execution_watch_hit(), "case={case}");
        assert_eq!(usize::from(word(&source, 0x144)), native.len());
        for (index, point) in native.iter().enumerate() {
            let actual: [i16; 3] =
                std::array::from_fn(|axis| word(&source, 0x4F0 + index * 6 + axis * 2) as i16);
            assert_eq!(*point, actual, "case={case} point={index}");
        }
    }
}

#[test]
fn authored_shape_rotation_to_bsp_matches_original_pipeline() {
    use sf2_data::{shape_program::FaceCommand, shape_program_data::FACE_PROGRAMS};
    use sf2_game::{
        intro_projection::{project_points, ProjectionViewport},
        intro_visibility::submit_projected_bsp,
    };
    assert_eq!(SHAPE_DATA.len(), POINT_PROGRAMS.len());
    assert_eq!(SHAPE_DATA.len(), FACE_PROGRAMS.len());
    let viewport = ProjectionViewport {
        center: [112, 96],
        left: 0,
        right: 224,
        top: 0,
        bottom: 192,
    };
    let mut source = Gsu::new(rom());
    let mut comparisons = 0;
    for ((shape, points), faces) in SHAPE_DATA.iter().zip(&POINT_PROGRAMS).zip(&FACE_PROGRAMS) {
        if !faces
            .nodes
            .iter()
            .any(|node| matches!(node.command, FaceCommand::BeginBsp { .. }))
        {
            continue;
        }
        for (scene, translation) in [[0, 0, 2048], [0, 0, 256], [400, -200, 128], [0, 0, -512]]
            .into_iter()
            .enumerate()
        {
            let columns = MATRICES[scene];
            let animation = (scene * 31) as u16;
            let rotated = transform_shape(shape, points, animation, columns).unwrap();
            let projected = project_points(&rotated, translation, viewport);
            let native = submit_projected_bsp(faces, &projected).unwrap();
            prepare(&mut source, shape, animation, columns);
            for (address, value) in [
                (0x26, translation[0]),
                (0x28, translation[1]),
                (0x2A, translation[2]),
                (0x34, 112),
                (0x36, 96),
                (0x38, 0),
                (0x3A, 224),
                (0x3C, 0),
                (0x3E, 192),
                (0x54, 0),
            ] {
                put_word(&mut source, address, value as u16);
            }
            put_word(&mut source, 0x18, shape.faces_address as u16);
            source.watch_execution(1, 0x9CC4);
            source.start(1, 0x96BA);
            while !source.execution_watch_hit()
                && source.r[15] != 0xA0C7
                && source.is_running()
                && source.last_run_steps < 100_000
            {
                source.run_slice(1);
            }
            for (index, point) in projected.iter().enumerate() {
                assert_eq!(
                    [point.x as u16, point.y as u16, point.outcode],
                    std::array::from_fn(|axis| word(&source, 0x6D0 + index * 6 + axis * 2)),
                    "shape={} scene={scene} point={index}",
                    shape.header_index
                );
            }
            if let Some(native) = native {
                assert!(
                    source.execution_watch_hit(),
                    "shape={} scene={scene}",
                    shape.header_index
                );
                for (index, flag) in native.visibility.iter().enumerate() {
                    assert_eq!(flag.0, source.ram[0xA50 + index]);
                }
                let end = usize::from(word(&source, 0x56));
                assert!((0x9B0..=0x9F0).contains(&end));
                assert_eq!(end % 2, 0);
                let actual: Vec<_> = source.ram[0x9B0..end]
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect();
                let expected: Vec<_> = native
                    .bsp
                    .face_lists
                    .iter()
                    .map(|id| faces.node(*id).unwrap().source_address as u16)
                    .collect();
                assert_eq!(
                    actual, expected,
                    "shape={} scene={scene}",
                    shape.header_index
                );
            } else {
                assert_eq!(
                    source.r[15], 0xA0C7,
                    "shape={} scene={scene}",
                    shape.header_index
                );
            }
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 768);
}
