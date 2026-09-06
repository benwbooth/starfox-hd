use sf2_game::intro_projection::{project_point, project_points, ProjectionViewport};
use sf_oracle::gsu::Gsu;

const VIEWPORT: ProjectionViewport = ProjectionViewport {
    center: [112, 96],
    left: 0,
    right: 224,
    top: 0,
    bottom: 192,
};

fn rom() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM")
}

fn put_word(source: &mut Gsu, address: usize, value: u16) {
    source.ram[address..address + 2].copy_from_slice(&value.to_le_bytes());
}

fn prepare_source(
    source: &mut Gsu,
    points: &[[i16; 3]],
    translation: [i16; 3],
    viewport: ProjectionViewport,
) {
    for (address, value) in [
        (0x26, translation[0] as u16),
        (0x28, translation[1] as u16),
        (0x2A, translation[2] as u16),
        (0x34, viewport.center[0] as u16),
        (0x36, viewport.center[1] as u16),
        (0x38, viewport.left as u16),
        (0x3A, viewport.right as u16),
        (0x3C, viewport.top as u16),
        (0x3E, viewport.bottom as u16),
        (0x144, points.len() as u16),
    ] {
        put_word(source, address, value);
    }
    for (index, value) in points.iter().flatten().copied().enumerate() {
        put_word(source, 0x4F0 + index * 2, value as u16);
    }
    source.r[10] = 0x400;
}

fn projected_record(source: &Gsu, index: usize) -> [u16; 3] {
    std::array::from_fn(|i| {
        u16::from_le_bytes([
            source.ram[0x6D0 + index * 6 + i * 2],
            source.ram[0x6D1 + index * 6 + i * 2],
        ])
    })
}

fn project_source(source: &mut Gsu, point: [i16; 3]) -> [u16; 3] {
    prepare_source(source, &[point], [0; 3], VIEWPORT);
    source.watch_execution(1, 0x9B6A);
    source.start(1, 0x9A99);
    while !source.execution_watch_hit() && source.is_running() && source.last_run_steps < 10_000 {
        source.run_slice(1);
    }
    assert!(source.execution_watch_hit(), "point={point:?}");
    projected_record(source, 0)
}

#[test]
fn retail_reciprocal_table_matches_integer_source_formula() {
    let rom = rom();
    // GSU ROM bank $19, address $BAB8. Include the exact far-depth limit:
    // the original BPL clamp retains z == $3000 and only clamps values above it.
    for depth in (0..=0x3000usize).step_by(2) {
        let address = 0xCBAB8 + depth;
        let actual = u16::from_le_bytes([rom[address], rom[address + 1]]);
        let expected = if depth < 256 {
            32767
        } else {
            (32767 * 256 / depth) as u16
        };
        assert_eq!(actual, expected, "depth={depth}");
    }
}

#[test]
fn retail_projection_distinguishes_near_and_table_edge_rules() {
    let mut source = Gsu::new(rom());
    // Table projection uses high16 only; older source's extra ROL is absent.
    assert_eq!(
        project_source(&mut source, [256, 0, 512]),
        [175, 96, 0x1F00]
    );
    // The near path uses bounded division, including zero-depth handling.
    assert_eq!(project_source(&mut source, [0, 0, 0]), [112, 96, 0x1F00]);
    for (point, expected) in [
        ([-224, 0, 256], [0, 96, 0x1B04]),
        ([224, 0, 256], [224, 96, 0x1F00]),
        ([0, -192, 256], [112, 0, 0x1E01]),
        ([0, 192, 256], [112, 192, 0x1F00]),
    ] {
        assert_eq!(project_source(&mut source, point), expected);
    }
}

#[test]
fn native_projection_matches_original_across_depth_and_coordinate_edges() {
    let mut source = Gsu::new(rom());
    let edges = [
        i16::MIN,
        -20481,
        -20480,
        -20479,
        -16384,
        -257,
        -256,
        -1,
        0,
        1,
        255,
        256,
        257,
        12287,
        12288,
        12289,
        i16::MAX,
    ];
    let mut compare = |point| {
        let actual = project_source(&mut source, point);
        let native = project_point(point, VIEWPORT);
        assert_eq!(
            [native.x as u16, native.y as u16, native.outcode],
            actual,
            "point={point:?}"
        );
    };
    for x in edges {
        for y in edges {
            for z in edges {
                compare([x, y, z]);
            }
        }
    }
    let mut random = 0x579A_CCE1u32;
    for _ in 0..32768 {
        compare(std::array::from_fn(|_| {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            random as i16
        }));
    }
    for depth in 0..=u16::MAX {
        compare([-32768, 16385, depth as i16]);
    }
}

#[test]
fn translated_projection_matches_original_with_varied_viewports() {
    let mut source = Gsu::new(rom());
    let mut random = 0xB39C_0117u32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random as i16
    };
    for case in 0..4096 {
        let viewport = ProjectionViewport {
            center: [next(), next()],
            left: next(),
            right: next(),
            top: next(),
            bottom: next(),
        };
        let translation = [next(), next(), next()];
        let points: Vec<_> = (0..4).map(|_| [next(), next(), next()]).collect();
        let native = project_points(&points, translation, viewport);
        prepare_source(&mut source, &points, translation, viewport);
        source.watch_execution(1, 0x9B6A);
        source.start(1, 0x9A99);
        while !source.execution_watch_hit() && source.is_running() && source.last_run_steps < 10_000
        {
            source.run_slice(1);
        }
        assert!(source.execution_watch_hit(), "case={case}");
        for (index, native) in native.iter().enumerate() {
            assert_eq!(
                [native.x as u16, native.y as u16, native.outcode],
                projected_record(&source, index),
                "case={case} index={index}"
            );
        }
        let aggregate = native.iter().fold(0u16, |bits, point| bits | point.outcode);
        assert_eq!(
            aggregate,
            u16::from_le_bytes([source.ram[0x14A], source.ram[0x14B]])
        );
    }
}

#[test]
fn camera_axis_points_to_bsp_matches_original_for_every_bsp_shape() {
    use sf2_data::{
        shape_data::SHAPE_DATA, shape_program::FaceCommand, shape_program_data::FACE_PROGRAMS,
    };
    use sf2_game::intro_visibility::submit_projected_bsp;
    let mut source = Gsu::new(rom());
    let mut comparisons = 0;
    for (shape, program) in SHAPE_DATA.iter().zip(&FACE_PROGRAMS) {
        if !program
            .nodes
            .iter()
            .any(|node| matches!(node.command, FaceCommand::BeginBsp { .. }))
        {
            continue;
        }
        // Authored vertex values are supplied at the already-rotated input
        // boundary; shape scaling and camera matrix construction are not tested.
        let points: Vec<_> = shape.vertices.iter().map(|v| [v.x, v.y, v.z]).collect();
        for translation in [[0, 0, 2048], [0, 0, 256], [400, -200, 128], [0, 0, -512]] {
            let projected = project_points(&points, translation, VIEWPORT);
            let native = submit_projected_bsp(program, &projected).unwrap();
            prepare_source(&mut source, &points, translation, VIEWPORT);
            put_word(&mut source, 0x18, shape.faces_address as u16);
            put_word(&mut source, 0x1C, (shape.faces_address >> 16) as u16);
            put_word(&mut source, 0x54, 0);
            source.watch_execution(1, 0x9CC4);
            source.start(1, 0x9A99);
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
                    projected_record(&source, index),
                    "shape={} index={index}",
                    shape.header_index
                );
            }
            if let Some(native) = native {
                assert!(
                    source.execution_watch_hit(),
                    "shape={} translation={translation:?}",
                    shape.header_index
                );
                for (index, flag) in native.visibility.iter().enumerate() {
                    assert_eq!(
                        flag.0,
                        source.ram[0xA50 + index],
                        "shape={} visibility={index}",
                        shape.header_index
                    );
                }
                let end = usize::from(u16::from_le_bytes([source.ram[0x56], source.ram[0x57]]));
                assert!((0x9B0..=0x9F0).contains(&end));
                let actual: Vec<_> = source.ram[0x9B0..end]
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect();
                let expected: Vec<_> = native
                    .bsp
                    .face_lists
                    .iter()
                    .map(|id| program.node(*id).unwrap().source_address as u16)
                    .collect();
                assert_eq!(
                    actual, expected,
                    "shape={} translation={translation:?}",
                    shape.header_index
                );
            } else {
                assert_eq!(
                    source.r[15], 0xA0C7,
                    "shape={} translation={translation:?}",
                    shape.header_index
                );
            }
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 768);
}
