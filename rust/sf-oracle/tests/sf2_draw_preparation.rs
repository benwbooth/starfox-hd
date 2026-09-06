//! Original full `$01:D28B` draw-list preparation, not an isolated dot product.
use sf2_data::shape_data::SHAPE_DATA;
use sf2_game::{
    intro_draw::{prepare_draw_list, DrawPlacement, ShadowPlacement, ViewTransform},
    Vector3,
};
use sf_oracle::gsu::Gsu;

const BASE: usize = 0xAD0;
const STRIDE: usize = 0x26;

fn put(bytes: &mut [u8], address: usize, value: u16) {
    bytes[address..address + 2].copy_from_slice(&value.to_le_bytes());
}

fn word(bytes: &[u8], address: usize) -> u16 {
    u16::from_le_bytes([bytes[address], bytes[address + 1]])
}

#[test]
fn native_draw_preparation_matches_original_full_list_job() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    let mut source = Gsu::new(rom);
    let mut random = 0x9317_D467u32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random as u16
    };
    let batches = SHAPE_DATA.len().div_ceil(64);
    let mut seen = vec![false; SHAPE_DATA.len()];
    let mut comparisons = 0;
    for case in 0..batches + 260 {
        let count = if case < batches {
            (SHAPE_DATA.len() - case * 64).min(64)
        } else {
            (case - batches) % 65
        };
        let mut view = ViewTransform {
            position: Vector3 {
                x: next() as i16,
                y: next() as i16,
                z: next() as i16,
            },
            matrix: std::array::from_fn(|_| std::array::from_fn(|_| next() as i16)),
        };
        let tied = case == batches + 4;
        let sort_edges = case == batches + 5;
        if tied || sort_edges {
            view.matrix = [[0; 3]; 3];
        }
        let height = next() as i16;
        put(&mut source.ram, 0xD8, view.position.x as u16);
        put(&mut source.ram, 0xDA, view.position.y as u16);
        put(&mut source.ram, 0xDC, view.position.z as u16);
        put(&mut source.ram, 0x3EE, height as u16);
        put(&mut source.ram, 0x1EE, (next() & 0xFF00) | count as u16);
        for (index, value) in view.matrix.into_iter().flatten().enumerate() {
            put(&mut source.ram, 0xE4 + index * 2, value as u16);
        }
        let mut expected = vec![0u8; count * STRIDE];
        for byte in &mut expected {
            *byte = next() as u8;
        }
        let mut inputs = Vec::with_capacity(count);
        for index in 0..count {
            let shape_index = if case < batches {
                case * 64 + index
            } else {
                usize::from(next()) % SHAPE_DATA.len()
            };
            seen[shape_index] = true;
            let shape = &SHAPE_DATA[shape_index];
            let record = &mut expected[index * STRIDE..(index + 1) * STRIDE];
            if tied || sort_edges {
                let key: i16 = if tied {
                    5
                } else {
                    [i16::MIN, i16::MAX, 0, -1, i16::MIN][index]
                };
                put(record, 2, key.wrapping_sub(shape.sort_z) as u16);
            }
            let flags = record[7];
            let object = DrawPlacement {
                position: Vector3 {
                    x: word(record, 0x20) as i16,
                    y: word(record, 0x22) as i16,
                    z: word(record, 0x24) as i16,
                },
                shadow: if flags & 4 != 0 {
                    ShadowPlacement::Object
                } else if flags & 8 != 0 {
                    ShadowPlacement::Ground { height }
                } else {
                    ShadowPlacement::None
                },
                shape_sort_bias: shape.sort_z,
                object_sort_bias: word(record, 2) as i16,
            };
            put(record, 8, shape.shape_id);
            inputs.push(object);
        }
        source.ram[BASE..BASE + expected.len()].copy_from_slice(&expected);
        let native = prepare_draw_list(view, &inputs);
        for (index, placement) in native.placements.iter().enumerate() {
            let record = &mut expected[index * STRIDE..(index + 1) * STRIDE];
            for (offset, value) in [
                (0x12, placement.position.x),
                (0x10, placement.position.y),
                (0x14, placement.position.z),
                (2, placement.sort_depth),
            ] {
                put(record, offset, value as u16);
            }
            if let Some(shadow) = placement.shadow {
                for (offset, value) in [(0xC, shadow.x), (0xA, shadow.y), (0xE, shadow.z)] {
                    put(record, offset, value as u16);
                }
            }
        }
        for (slot, &index) in native.order.iter().enumerate() {
            let next = native
                .order
                .get(slot + 1)
                .map_or(0, |index| (BASE + index * STRIDE) as u16);
            put(&mut expected, index * STRIDE, next);
        }
        source.r[11] = 0xCE35; // Original STOP after returning from D28B.
        source.run_with_limit(1, 0xD28B, 200_000);
        assert!(!source.is_running(), "case={case}");
        for index in 0..count {
            for offset in [2, 0xA, 0xC, 0xE, 0x10, 0x12, 0x14] {
                assert_eq!(
                    word(&source.ram, BASE + index * STRIDE + offset),
                    word(&expected, index * STRIDE + offset),
                    "case={case} object={index} field={offset:X}"
                );
            }
        }
        assert_eq!(
            word(&source.ram, 0x3EC),
            native
                .order
                .first()
                .map_or(0, |index| (BASE + index * STRIDE) as u16),
            "head case={case}"
        );
        assert_eq!(
            &source.ram[BASE..BASE + expected.len()],
            &expected,
            "all record bytes case={case}"
        );
        comparisons += count;
    }
    assert!(seen.into_iter().all(|seen| seen));
    assert_eq!(comparisons, SHAPE_DATA.len() + 4 * (0..65).sum::<usize>());
}
