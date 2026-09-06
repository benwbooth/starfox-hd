use sf2_data::shape_data::SHAPE_DATA;
use sf2_data::shape_program::{FaceCommand, NodeId};
use sf2_data::shape_program_data::FACE_PROGRAMS;
use sf2_game::intro_bsp_work::submit_bsp;
use sf_oracle::gsu::Gsu;

fn rom() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM")
}

fn offset(address: u32) -> usize {
    (address >> 16) as usize * 0x8000 + (address & 0x7FFF) as usize
}

#[test]
fn typed_face_graph_preserves_original_operands_and_geometry_ranges() {
    let rom = rom();
    assert_eq!(FACE_PROGRAMS.len(), SHAPE_DATA.len());
    assert_eq!(
        FACE_PROGRAMS.iter().map(|p| p.nodes.len()).sum::<usize>(),
        4037
    );
    for (shape, program) in SHAPE_DATA.iter().zip(&FACE_PROGRAMS) {
        let mut covered = vec![false; shape.faces.len()];
        if let Some(root) = program.root {
            assert_eq!(
                program.node(root).unwrap().source_address,
                shape.faces_address
            );
        } else {
            assert_eq!(shape.faces_address, 0);
            assert!(program.nodes.is_empty());
        }
        for node in program.nodes {
            let source = node.source_address;
            let bytes = &rom[offset(source)..];
            let target = |id: NodeId| {
                let address = program
                    .node(id)
                    .expect("resolved native edge")
                    .source_address;
                assert_eq!(address >> 16, source >> 16);
                address as u16
            };
            let next = |id: NodeId, length: u16| {
                assert_eq!(target(id), (source as u16).wrapping_add(length));
            };
            match node.command {
                FaceCommand::Visibility {
                    triangles,
                    next: id,
                } => {
                    assert_eq!(bytes[0], 0x30);
                    assert_eq!(usize::from(bytes[1]), triangles.len());
                    for (index, triangle) in triangles.iter().enumerate() {
                        assert_eq!(triangle.as_slice(), &bytes[2 + index * 3..5 + index * 3]);
                    }
                    next(id, 2 + 3 * triangles.len() as u16);
                }
                FaceCommand::BeginBsp { root } => {
                    assert_eq!(bytes[0], 0x3C);
                    next(root, 1);
                }
                FaceCommand::Bsp {
                    visibility,
                    coplanar,
                    left,
                    right,
                } => {
                    assert_eq!(bytes[0], 0x28);
                    assert_eq!(visibility, bytes[1]);
                    let delta = u16::from_le_bytes([bytes[2], bytes[3]]);
                    assert_eq!(
                        target(coplanar),
                        (source as u16).wrapping_add(3).wrapping_add(delta)
                    );
                    next(left, 5);
                    assert_eq!(
                        right.map(target),
                        (bytes[4] != 0)
                            .then(|| (source as u16).wrapping_add(4 + u16::from(bytes[4])))
                    );
                }
                FaceCommand::BspLeaf { faces } => {
                    assert_eq!(bytes[0], 0x44);
                    let delta = u16::from_le_bytes([bytes[1], bytes[2]]);
                    assert_eq!(
                        target(faces),
                        (source as u16).wrapping_add(2).wrapping_add(delta)
                    );
                }
                FaceCommand::ReturnBsp => assert_eq!(bytes[0], 0x40),
                FaceCommand::Quit => assert_eq!(bytes[0], 0x48),
                FaceCommand::EndShape => assert_eq!(bytes[0], 0),
                FaceCommand::Faces {
                    first,
                    count,
                    next: id,
                } => {
                    assert_eq!(bytes[0], 0x14);
                    let mut cursor = 1;
                    for index in usize::from(first)..usize::from(first) + usize::from(count) {
                        assert!(!covered[index]);
                        covered[index] = true;
                        let face = &shape.faces[index];
                        assert_eq!(face.num_verts, bytes[cursor]);
                        assert_eq!(face.color_index, bytes[cursor + 2]);
                        assert_eq!(
                            face.normal,
                            std::array::from_fn(|i| bytes[cursor + 3 + i] as i8)
                        );
                        for i in 0..usize::from(face.num_verts) {
                            assert_eq!(face.vertex_indices[i], u16::from(bytes[cursor + 6 + i]));
                        }
                        cursor += 6 + usize::from(face.num_verts);
                    }
                    assert_eq!(bytes[cursor], if id.is_some() { 0xFE } else { 0xFF });
                    if let Some(id) = id {
                        next(id, cursor as u16 + 1);
                    }
                }
                FaceCommand::ClipPlane { plane, next: id } => {
                    assert_eq!(bytes[0], 0x68);
                    assert_eq!(shape.clipping_planes[usize::from(plane)].slot, bytes[1]);
                    next(id, 14);
                }
                FaceCommand::Groups { entries } => {
                    assert_eq!(bytes[0], 0x10);
                    assert_eq!(usize::from(bytes[1]), entries.len());
                    for (index, entry) in entries.iter().enumerate() {
                        assert_eq!(entry.depth_point, bytes[2 + index]);
                        let cursor = 2 + entries.len() + index * 2;
                        assert_eq!(
                            target(entry.root),
                            u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]])
                        );
                    }
                }
                FaceCommand::Sprite {
                    parameters,
                    next: id,
                } => {
                    assert_eq!(bytes[0], 0x50);
                    assert_eq!(parameters.as_slice(), &bytes[1..4]);
                    next(id, 4);
                }
                FaceCommand::VisibleSprite {
                    parameters,
                    next: id,
                } => {
                    assert_eq!(bytes[0], 0x54);
                    assert_eq!(parameters.as_slice(), &bytes[1..5]);
                    next(id, 5);
                }
            }
        }
        assert!(covered.iter().all(|covered| *covered));
    }
}

#[test]
fn native_bsp_order_matches_original_handler_for_every_bsp_shape() {
    let rom = rom();
    let mut compared = 0;
    for (shape, program) in SHAPE_DATA.iter().zip(&FACE_PROGRAMS) {
        for (index, begin) in program.nodes.iter().enumerate() {
            if !matches!(begin.command, FaceCommand::BeginBsp { .. }) {
                continue;
            }
            for pattern in 0..4 {
                let flags: [bool; 256] = std::array::from_fn(|i| match pattern {
                    0 => false,
                    1 => true,
                    2 => i & 1 != 0,
                    _ => i % 3 == 0,
                });
                let native = submit_bsp(program, NodeId(index as u16), &flags).unwrap();
                let expected: Vec<_> = native
                    .face_lists
                    .iter()
                    .map(|id| program.node(*id).unwrap().source_address as u16)
                    .collect();
                let mut fixture = rom.clone();
                // Isolated entry adapter sets the shape data bank, then calls
                // the unchanged original BSP initializer/handlers. No render
                // frames, branch results or submitted lists are patched.
                fixture[0xFF00..0xFF08].copy_from_slice(&[
                    0xA0,
                    (begin.source_address >> 16) as u8,
                    0x3F,
                    0xDF,
                    0xFF,
                    0x3D,
                    0x9C,
                    0x01,
                ]);
                let mut source = Gsu::new(fixture);
                source.r[10] = 0x0400;
                source.r[14] = (begin.source_address as u16).wrapping_add(1);
                for (index, negative) in flags.iter().enumerate() {
                    source.ram[0x0A50 + index] = if *negative { 0x80 } else { 0 };
                }
                source.watch_execution(1, 0x9CC4);
                source.trace_next_run();
                source.start(1, 0xFF00);
                while !source.execution_watch_hit()
                    && source.is_running()
                    && source.last_run_steps < 100_000
                {
                    source.run_slice(1);
                }
                assert!(
                    source.execution_watch_hit(),
                    "shape={} pattern={pattern}",
                    shape.header_index
                );
                let end = u16::from_le_bytes([source.ram[0x56], source.ram[0x57]]);
                assert!((0x09B0..=0x09F0).contains(&end));
                let actual: Vec<_> = source.ram[0x09B0..usize::from(end)]
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect();
                assert_eq!(
                    actual, expected,
                    "shape={} pattern={pattern}",
                    shape.header_index
                );
                let trace = source.pc_trace();
                assert_eq!(
                    trace.iter().filter(|pc| **pc == 0x019C60).count(),
                    native.branches.len()
                );
                assert_eq!(
                    trace.iter().filter(|pc| **pc == 0x019C4D).count(),
                    native.leaves as usize
                );
                assert_eq!(
                    trace.iter().filter(|pc| **pc == 0x019C5B).count(),
                    (native.leaves + native.returns) as usize
                );
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 192 * 4);
}
