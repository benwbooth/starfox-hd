//! Execute the original SF2 view-matrix and camera-relative rotation jobs.
//! These checks certify arithmetic, not scene input selection or scheduling.
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
