//! Full CPU camera selection and all five original GSU math jobs.
use sf2_data::shape_data::SHAPE_DATA;
use sf2_game::{
    intro_camera::IntroCameraView,
    intro_draw::{DrawPlacement, ShadowPlacement, ViewTransform},
    intro_motion::AttractCameraAngles,
    oracle_compat::Game,
    Vector3,
};
use sf_oracle::gsu::Gsu;

#[test]
fn native_camera_view_matches_both_original_camera_handoffs() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    let mut draw_source = Gsu::new(rom.clone());
    let mut source = Game::new(rom).unwrap();
    let mut random = 0xB72F_18D9u32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random as u16
    };
    for case in 0..2048 {
        let mut cameras = Vec::new();
        for (index, object) in [0x033F, 0x037E].into_iter().enumerate() {
            let camera = IntroCameraView {
                position: Vector3 {
                    x: next() as i16,
                    y: next() as i16,
                    z: next() as i16,
                },
                angles: AttractCameraAngles {
                    pitch: next(),
                    yaw: next(),
                    roll: next(),
                },
            };
            let distance = if case < 12 {
                [0, 1, -1, 256, i16::MIN, i16::MAX][case / 2]
            } else {
                next() as i16
            };
            let auxiliary = 0x140 + index as u16 * 0x100;
            source.memory.write_word(object + 6, auxiliary);
            source.memory.write_word(auxiliary + 0x2D, next());
            for (field, value) in [
                (0xC, camera.position.x as u16),
                (0xE, camera.position.y as u16),
                (0x10, camera.position.z as u16),
                (0x12, camera.angles.pitch),
                (0x14, camera.angles.yaw),
                (0x16, camera.angles.roll),
                (0x29, distance as u16),
            ] {
                source.memory.write_word(object + field, value);
            }
            cameras.push((camera, distance));
        }
        source.memory.write_word(0xCF19, next());
        source.memory.write_word(0xCF1B, next());
        source.memory.write_word(0xCF1D, 1);
        let selected = case % 2;
        source
            .run_retail_oracle_routine(if selected == 0 { 0x7F1561 } else { 0x7F156A }, 0)
            .unwrap();
        let (camera, distance) = cameras[selected];
        let native = ViewTransform::from_camera(camera, distance);
        assert_eq!(source.memory.read_word(0x1934), [0x33F, 0x37E][selected]);
        for (address, coordinate) in [
            (0xD8, native.position.x),
            (0xDA, native.position.y),
            (0xDC, native.position.z),
        ] {
            assert_eq!(
                source.memory.read_long_word(0x700000 + address) as i16,
                coordinate,
                "case={case} position={address:X}"
            );
        }
        for (index, coefficient) in native.matrix.into_iter().flatten().enumerate() {
            assert_eq!(
                source.memory.read_long_word(0x7000E4 + index as u32 * 2) as i16,
                coefficient,
                "case={case} coefficient={index}"
            );
            assert_eq!(
                source.memory.read_word(0x157C + index as u16 * 2) as i16,
                coefficient,
                "case={case} saved coefficient={index}"
            );
        }
        // Continue with the actual source handoff's RAM, not native-derived
        // matrix/position seeds, through the complete draw preparation pass.
        for (address, byte) in draw_source.ram[..0x2000].iter_mut().enumerate() {
            *byte = source.memory.read_long_byte(0x700000 + address as u32);
        }
        let shape = &SHAPE_DATA[case % SHAPE_DATA.len()];
        let placement = DrawPlacement {
            position: Vector3 {
                x: next() as i16,
                y: next() as i16,
                z: next() as i16,
            },
            shadow: ShadowPlacement::Ground {
                height: source.memory.read_word(0xCF19) as i16,
            },
            shape_sort_bias: shape.sort_z,
            object_sort_bias: next() as i16,
        };
        let record = &mut draw_source.ram[0xAD0..0xAF6];
        record.fill(0);
        record[7] = 8;
        for (address, value) in [
            (2, placement.object_sort_bias as u16),
            (8, shape.shape_id),
            (0x20, placement.position.x as u16),
            (0x22, placement.position.y as u16),
            (0x24, placement.position.z as u16),
        ] {
            record[address..address + 2].copy_from_slice(&value.to_le_bytes());
        }
        draw_source.r[11] = 0xCE35;
        draw_source.run_with_limit(1, 0xD28B, 5000);
        assert!(!draw_source.is_running(), "draw case={case}");
        let prepared = native.prepare(placement);
        let shadow = prepared.shadow.unwrap();
        for (offset, value) in [
            (2, prepared.sort_depth),
            (0x12, prepared.position.x),
            (0x10, prepared.position.y),
            (0x14, prepared.position.z),
            (0xC, shadow.x),
            (0xA, shadow.y),
            (0xE, shadow.z),
        ] {
            let offset = 0xAD0 + offset;
            assert_eq!(
                i16::from_le_bytes([draw_source.ram[offset], draw_source.ram[offset + 1]]),
                value,
                "camera to placement case={case} field={offset:X}"
            );
        }
    }
}
