//! Original edge-crossing routine and its signed weighted-coordinate divider.
use sf2_data::shape_data::ShapeVertex;
use sf_oracle::gsu::Gsu;
use sf_render::sf2_clipping::{edge_intersection, ClipVertex};

fn put(source: &mut Gsu, address: usize, value: u16) {
    source.ram[address..address + 2].copy_from_slice(&value.to_le_bytes());
}

fn word(source: &Gsu, address: usize) -> u16 {
    u16::from_le_bytes(source.ram[address..address + 2].try_into().unwrap())
}

#[test]
fn every_plane_distance_matches_the_original_edge_intersection() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    let mut source = Gsu::new(rom);
    let mut random = 0x8731_AB09u32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random as i16
    };
    let mut crossings = 0;
    let mut same_side = 0;
    for bits in u16::MIN..=u16::MAX {
        for other in [i16::MIN, -1, 0, 1, i16::MAX] {
            let first = ClipVertex {
                position: ShapeVertex {
                    x: next(),
                    y: next(),
                    z: next(),
                },
                distance: bits as i16,
            };
            let second = ClipVertex {
                position: ShapeVertex {
                    x: next(),
                    y: next(),
                    z: next(),
                },
                distance: other,
            };
            for (index, vertex) in [first, second].into_iter().enumerate() {
                for (field, value) in [
                    vertex.distance,
                    vertex.position.x,
                    vertex.position.y,
                    vertex.position.z,
                ]
                .into_iter()
                .enumerate()
                {
                    put(&mut source, 0x24E4 + index * 8 + field * 2, value as u16);
                }
            }
            source.ram[0x8B0..0x8B8].fill(0xA5);
            source.r[8] = 0x8B0;
            source.r[9] = 0x24E4;
            source.r[10] = 0x3F0;
            source.r[11] = 0xCE35; // Original STOP, reached only on return.
            source.r[14] = 7;
            source.run_with_limit(1, 0xF5AE, 4096);
            assert!(!source.is_running(), "edge {bits} {other}");
            assert_eq!(source.r[9], 0x24EC);
            assert_eq!(source.r[10], 0x3F0);
            if let Some(intersection) = edge_intersection(first, second) {
                crossings += 1;
                assert_eq!(source.r[8], 0x8B6);
                assert_eq!(source.r[14], 8);
                for (field, expected) in [intersection.x, intersection.y, intersection.z]
                    .into_iter()
                    .enumerate()
                {
                    assert_eq!(
                        word(&source, 0x8B0 + field * 2) as i16,
                        expected,
                        "distance={} other={other} field={field} first={first:?} second={second:?}",
                        bits as i16
                    );
                }
                assert_eq!(word(&source, 0x8B6), 0xA5A5);
            } else {
                same_side += 1;
                assert_eq!(source.r[8], 0x8B0);
                assert_eq!(source.r[14], 7);
                assert_eq!(&source.ram[0x8B0..0x8B8], &[0xA5; 8]);
            }
        }
    }
    assert_eq!(crossings, 163_840);
    assert_eq!(same_side, 163_840);
}
