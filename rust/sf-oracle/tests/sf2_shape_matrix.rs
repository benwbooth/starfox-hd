//! Execute the original SF2 view-matrix and camera-relative rotation jobs.
//! These checks certify arithmetic, not scene input selection or scheduling.
use sf2_game::intro_transform::{face_shade_index, object_light_direction, object_view_matrix};
use sf_core::snes_trig::{matrix_rotate_q15, zxy_matrix_q15_fine};
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

fn run_job(source: &mut Gsu, entry: u16) {
    source.run_with_limit(1, entry, 5000);
    assert!(!source.is_running(), "entry={entry:04X}");
}

#[test]
fn native_fine_view_matrix_matches_original_job() {
    let mut source = Gsu::new(rom());
    let mut compare = |angles: [u16; 3]| {
        for (index, angle) in angles.into_iter().enumerate() {
            put_word(&mut source, 0x20 + index * 2, angle);
        }
        run_job(&mut source, 0x9191);
        let native = zxy_matrix_q15_fine(angles[0], angles[1], angles[2]);
        let actual: [[i16; 3]; 3] = std::array::from_fn(|row| {
            std::array::from_fn(|column| word(&source, 0xE4 + row * 6 + column * 2) as i16)
        });
        assert_eq!(native, actual, "angles={angles:?}");
    };
    for axis in 0..3 {
        for angle in 0..=u16::MAX {
            let mut angles = [0x742D, 0xBE17, 0x53E1];
            angles[axis] = angle;
            compare(angles);
        }
    }
    let mut random = 0x742D_9563u32;
    for _ in 0..8192 {
        compare(std::array::from_fn(|_| {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            random as u16
        }));
    }
}

#[test]
fn native_relative_translation_matches_original_world_rotation_job() {
    let mut source = Gsu::new(rom());
    let mut random = 0xB837_7165u32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random as i16
    };
    for case in 0..16384 {
        let matrix = std::array::from_fn(|_| std::array::from_fn(|_| next()));
        let [x, y, z] = [next(), next(), next()];
        for (index, coefficient) in matrix.into_iter().flatten().enumerate() {
            put_word(&mut source, 0xE4 + index * 2, coefficient as u16);
        }
        put_word(&mut source, 0x68, x as u16);
        put_word(&mut source, 0x2C, y as u16);
        put_word(&mut source, 0x2E, z as u16);
        run_job(&mut source, 0x913A);
        let native = matrix_rotate_q15(matrix, x, y, z);
        assert_eq!(
            [native.0, native.1, native.2],
            std::array::from_fn(|axis| word(&source, 0x26 + axis * 2) as i16),
            "case={case}"
        );
    }
}

#[test]
fn native_object_view_composition_matches_original() {
    let mut source = Gsu::new(rom());
    let mut random = 0x7A46_53D1u32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random as u16
    };
    let special = [
        [0, 0, 0],
        [0, 128, 0],
        [0, 64, 0],
        [256, 0, 0],
        [0, 256, 0],
        [0, 0, 256],
        [0, 384, 0],
    ];
    for case in 0..16384 {
        let view = std::array::from_fn(|_| std::array::from_fn(|_| next() as i16));
        let angles = if case < special.len() * 2 {
            special[case / 2]
        } else if case < 1550 {
            let mut angles = [0; 3];
            angles[(case - 14) / 512] = ((case - 14) / 2 % 256) as u16;
            angles
        } else if case < 8192 {
            std::array::from_fn(|_| next() & 255)
        } else {
            std::array::from_fn(|_| next())
        };
        let flags = (next() & !0x2000) | if case & 1 != 0 { 0x2000 } else { 0 };
        for (index, coefficient) in view.into_iter().flatten().enumerate() {
            put_word(&mut source, 0xE4 + index * 2, coefficient as u16);
        }
        for (index, angle) in angles.into_iter().enumerate() {
            put_word(&mut source, 0x20 + index * 2, angle);
        }
        put_word(&mut source, 0x32, (case % 16) as u16);
        put_word(&mut source, 0x54, flags);
        source.r[10] = 0x400;
        source.watch_execution(1, 0x964E);
        source.start(1, 0x9528);
        while !source.execution_watch_hit() && source.is_running() && source.last_run_steps < 5000 {
            source.run_slice(1);
        }
        assert!(source.execution_watch_hit(), "case={case}");
        let actual: [[i16; 3]; 3] = std::array::from_fn(|input| {
            std::array::from_fn(|output| word(&source, 0x132 + input * 6 + output * 2) as i16)
        });
        assert_eq!(
            object_view_matrix(view, angles, flags),
            actual,
            "case={case} angles={angles:?} flags={flags:04X}"
        );
    }
}

#[test]
fn native_object_light_direction_matches_original() {
    let mut source = Gsu::new(rom());
    let mut random = 0x964E_49E5u32;
    for case in 0..73728 {
        let mut matrix = std::array::from_fn(|_| {
            std::array::from_fn(|_| {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                random as i16
            })
        });
        // Sweep every word through each row, retaining different nonsymmetric
        // companion coefficients. Random matrices also exercise wrapping sums.
        if case < 65536 {
            for axis in 0..3 {
                matrix[axis][axis] = case as i16;
            }
        }
        for (index, coefficient) in matrix.into_iter().flatten().enumerate() {
            put_word(&mut source, 0x132 + index * 2, coefficient as u16);
        }
        // Watches observe and execute the marked instruction. Stop on the
        // final store, before the next routine's ALT1 prefix is consumed.
        source.watch_execution(1, 0x96B8);
        source.start(1, 0x964E);
        while !source.execution_watch_hit() && source.is_running() && source.last_run_steps < 500 {
            source.run_slice(1);
        }
        assert!(source.execution_watch_hit(), "case={case}");
        assert_eq!(
            object_light_direction(matrix).map(i16::from),
            std::array::from_fn(|axis| word(&source, 0x106 + axis * 2) as i16),
            "case={case} matrix={matrix:?}"
        );
    }
}

#[test]
fn native_face_shading_matches_both_original_consumers() {
    let cartridge = rom();
    let mut source = Gsu::new(cartridge.clone());
    let mut random = 0x9EC0_A1CDu32;
    let mut levels_seen = [false; 10];
    let mut wrapping_changes = 0;
    for case in 0..65536 {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        // Read genuine ROM bytes for the two GETB normal operands, keeping
        // the source stream unmodified. ROMB defaults to bank zero.
        let address = (random as usize % 32767) as u16;
        let normal = [
            case as i8,
            cartridge[address as usize] as i8,
            cartridge[address as usize + 1] as i8,
        ];
        let light = [
            (case >> 8) as i8,
            (random >> 16) as i8,
            (random >> 24) as i8,
        ];
        let shade = face_shade_index(normal, light);
        levels_seen[usize::from(shade)] = true;
        let wide_dot: i32 = normal
            .into_iter()
            .zip(light)
            .map(|(normal, light)| i32::from(normal) * i32::from(light))
            .sum();
        wrapping_changes += usize::from(i32::from(shade) != (wide_dot >> 10).clamp(6, 15) - 6);
        for (axis, component) in light.into_iter().enumerate() {
            put_word(&mut source, 0x106 + axis * 2, component as i16 as u16);
        }
        for (entry, finish) in [(0x9EC0, 0x9EEE), (0xA1CD, 0xA1FB)] {
            source.r[0] = normal[0] as u8 as u16;
            source.r[14] = address;
            source.watch_execution(1, finish);
            source.start(1, entry);
            while !source.execution_watch_hit()
                && source.is_running()
                && source.last_run_steps < 500
            {
                source.run_slice(1);
            }
            assert!(
                source.execution_watch_hit(),
                "case={case} entry={entry:04X}"
            );
            assert_eq!(
                u16::from(shade),
                source.r[1],
                "case={case} entry={entry:04X} normal={normal:?} light={light:?}"
            );
        }
    }
    assert!(levels_seen.into_iter().all(|seen| seen));
    assert!(
        wrapping_changes > 0,
        "must distinguish widened dot products"
    );
}
