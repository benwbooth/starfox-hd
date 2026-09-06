use sf2_game::intro_visibility::{
    select_visibility_mode, submit_projected_bsp, triangle_visibility, ProjectedShapePoint,
    ShapeVisibility, VisibilityMode,
};
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

#[test]
fn visibility_bytes_match_both_original_arithmetic_paths() {
    let mut fixture = rom();
    fixture[0x7000..0x7005].copy_from_slice(&[1, 0, 1, 2, 0]);
    let mut source = Gsu::new(fixture);
    let mut random = 0xC851_06A7u32;
    for mode in [VisibilityMode::FullPrecision, VisibilityMode::OnScreen] {
        for case in 0..8192 {
            let mut next = || {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                random as u16
            };
            let points = std::array::from_fn(|_| ProjectedShapePoint {
                x: next() as i16,
                y: next() as i16,
                outcode: next(),
            });
            for (index, point) in points.iter().enumerate() {
                put_word(&mut source, 0x06D0 + index * 6, point.x as u16);
                put_word(&mut source, 0x06D2 + index * 6, point.y as u16);
                put_word(&mut source, 0x06D4 + index * 6, point.outcode);
            }
            put_word(
                &mut source,
                0x54,
                if mode == VisibilityMode::OnScreen {
                    0x4000
                } else {
                    0
                },
            );
            source.r[14] = 0xF000;
            source.watch_execution(
                1,
                if mode == VisibilityMode::OnScreen {
                    0xA2CB
                } else {
                    0xA280
                },
            );
            source.start(1, 0xA212);
            while !source.execution_watch_hit()
                && source.is_running()
                && source.last_run_steps < 1000
            {
                source.run_slice(1);
            }
            assert!(source.execution_watch_hit(), "mode={mode:?} case={case}");
            assert_eq!(
                triangle_visibility(points, mode),
                ShapeVisibility(source.ram[0x0A50]),
                "mode={mode:?} case={case} points={points:?}"
            );
            assert_eq!(source.r[11], 0x0A51);
        }
    }
}

#[test]
fn culling_and_mode_selection_match_original_for_every_aggregate_word() {
    let mut source = Gsu::new(rom());
    for aggregate in 0..=u16::MAX {
        put_word(&mut source, 0x014A, aggregate);
        put_word(&mut source, 0x0054, 0x1234);
        source.start(1, 0x9B6A);
        while !matches!(source.r[15], 0x9B8B | 0xA0C7)
            && source.is_running()
            && source.last_run_steps < 100
        {
            source.run_slice(1);
        }
        assert!(
            matches!(source.r[15], 0x9B8B | 0xA0C7),
            "aggregate={aggregate:04X}"
        );
        let actual = if source.r[15] == 0xA0C7 {
            None
        } else if source.ram[0x55] & 0x40 != 0 {
            Some(VisibilityMode::OnScreen)
        } else {
            Some(VisibilityMode::FullPrecision)
        };
        let native = select_visibility_mode(&[ProjectedShapePoint {
            outcode: aggregate,
            ..Default::default()
        }]);
        assert_eq!(native, actual, "aggregate={aggregate:04X}");
        let flags = u16::from_le_bytes([source.ram[0x54], source.ram[0x55]]);
        assert_eq!(flags & !0x4000, 0x1234, "aggregate={aggregate:04X}");
    }
}

#[test]
fn projected_visibility_and_bsp_pipeline_matches_original_for_all_bsp_shapes() {
    use sf2_data::shape_data::SHAPE_DATA;
    use sf2_data::shape_program::FaceCommand;
    use sf2_data::shape_program_data::FACE_PROGRAMS;
    let rom = rom();
    let mut comparisons = 0;
    for (shape, program) in SHAPE_DATA.iter().zip(&FACE_PROGRAMS) {
        if !program
            .nodes
            .iter()
            .any(|node| matches!(node.command, FaceCommand::BeginBsp { .. }))
        {
            continue;
        }
        for scene in 0..4 {
            let mut points: Vec<_> = (0..shape.vertices.len())
                .map(|i| ProjectedShapePoint {
                    x: ((i * 19 + 37) % 224 + 16) as i16,
                    y: ((i * 31 + 19) % 160 + 8) as i16,
                    outcode: 0x1F00,
                })
                .collect();
            match scene {
                1 => {
                    points[0].x = -1;
                    points[0].outcode = 0x1B04;
                }
                2 => {
                    points[0].x = -points[0].x;
                    points[0].y = -points[0].y;
                    points[0].outcode = 0x0F10;
                }
                3 => {
                    for point in &mut points {
                        point.outcode = 0x0F10;
                    }
                }
                _ => {}
            }
            let native = submit_projected_bsp(program, &points).unwrap();
            let mut fixture = rom.clone();
            // Supply ordinary mesh state at the post-projection boundary,
            // then execute the source cull -> mode -> visibility -> BSP path.
            // The adapter only selects the authored face-data bank and entry.
            fixture[0xFF00..0xFF08].copy_from_slice(&[
                0xA0,
                (shape.faces_address >> 16) as u8,
                0x3F,
                0xDF,
                0xFF,
                0x6A,
                0x9B,
                0x01,
            ]);
            let mut source = Gsu::new(fixture);
            let mut aggregate = 0;
            for (index, point) in points.iter().enumerate() {
                put_word(&mut source, 0x06D0 + index * 6, point.x as u16);
                put_word(&mut source, 0x06D2 + index * 6, point.y as u16);
                put_word(&mut source, 0x06D4 + index * 6, point.outcode);
                aggregate |= point.outcode;
            }
            put_word(&mut source, 0x014A, aggregate);
            put_word(&mut source, 0x0018, shape.faces_address as u16);
            put_word(&mut source, 0x0054, 0);
            source.r[10] = 0x0400;
            source.watch_execution(1, 0x9CC4);
            source.start(1, 0xFF00);
            while !source.execution_watch_hit()
                && source.r[15] != 0xA0C7
                && source.is_running()
                && source.last_run_steps < 100_000
            {
                source.run_slice(1);
            }
            if let Some(native) = native {
                assert!(
                    source.execution_watch_hit(),
                    "shape={} scene={scene}",
                    shape.header_index
                );
                let source_mode = if source.ram[0x55] & 0x40 != 0 {
                    VisibilityMode::OnScreen
                } else {
                    VisibilityMode::FullPrecision
                };
                assert_eq!(native.mode, source_mode);
                for (index, flag) in native.visibility.iter().enumerate() {
                    assert_eq!(
                        flag.0,
                        source.ram[0x0A50 + index],
                        "visibility shape={} scene={scene} index={index}",
                        shape.header_index
                    );
                }
                let end = usize::from(u16::from_le_bytes([source.ram[0x56], source.ram[0x57]]));
                assert!((0x09B0..=0x09F0).contains(&end));
                let actual: Vec<_> = source.ram[0x09B0..end]
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
                    "BSP shape={} scene={scene}",
                    shape.header_index
                );
            } else {
                assert_eq!(
                    source.r[15], 0xA0C7,
                    "cull shape={} scene={scene}",
                    shape.header_index
                );
            }
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 192 * 4);
}
