//! The original individual-point projector uses bounded division at every
//! depth. It is deliberately not the mesh projector's reciprocal-table path.

use sf2_game::intro_projection::{project_individual_point, ProjectionViewport};
use sf_oracle::gsu::Gsu;

fn word(source: &Gsu, address: usize) -> u16 {
    u16::from_le_bytes([source.ram[address], source.ram[address + 1]])
}

fn put(source: &mut Gsu, address: usize, value: u16) {
    source.ram[address..address + 2].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn individual_point_projection_matches_original_at_all_depths_and_signed_coordinate_edges() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 ROM required");
    let mut source = Gsu::new(rom);
    let default_view = ProjectionViewport {
        center: [112, 96],
        left: 0,
        right: 224,
        top: 0,
        bottom: 192,
    };
    let mut compare = |point: [i16; 3], viewport: ProjectionViewport| {
        for (address, value) in [
            (0x68, point[0]),
            (0x2C, point[1]),
            (0x2E, point[2]),
            (0x34, viewport.center[0]),
            (0x36, viewport.center[1]),
            (0x38, viewport.left),
            (0x3A, viewport.right),
            (0x3C, viewport.top),
            (0x3E, viewport.bottom),
        ] {
            put(&mut source, address, value as u16);
        }
        source.run_with_limit(1, 0xD50A, 5000);
        assert!(!source.is_running(), "point={point:?}");
        let native = project_individual_point(point, viewport);
        assert_eq!(
            [native.x as u16, native.y as u16],
            [word(&source, 0x68), word(&source, 0x2C)],
            "{point:?} {viewport:?}"
        );
        assert_eq!(word(&source, 0x2E), point[2] as u16);
        assert_eq!(native.outcode, source.r[0], "{point:?} {viewport:?}");
    };
    let edges = [
        i16::MIN,
        -32767,
        -16385,
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
        32767,
    ];
    for x in edges {
        for y in edges {
            for z in edges {
                compare([x, y, z], default_view);
            }
        }
    }
    for depth in 0..=u16::MAX {
        compare([-32768, 16385, depth as i16], default_view);
    }
    let mut random = 0xA4E7_C193u32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random as i16
    };
    for _ in 0..16384 {
        compare(
            [next(), next(), next()],
            ProjectionViewport {
                center: [next(), next()],
                left: next(),
                right: next(),
                top: next(),
                bottom: next(),
            },
        );
    }
}
