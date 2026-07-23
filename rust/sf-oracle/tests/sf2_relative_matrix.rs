//! SF2 GSU oracle for the attachment matrix used by path opcode `$0A2`.

use sf_core::snes_trig::{matrix_rotate_q15, zxy_matrix_q15};
use sf_oracle::gsu::Gsu;

fn retail_sf2() -> Option<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)?;
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc")).ok()
}

fn write_word(gsu: &mut Gsu, address: usize, value: i16) {
    let bytes = (value as u16).to_le_bytes();
    gsu.ram[address] = bytes[0];
    gsu.ram[address + 1] = bytes[1];
}

fn read_word(gsu: &Gsu, address: usize) -> i16 {
    i16::from_le_bytes([gsu.ram[address], gsu.ram[address + 1]])
}

fn oracle_matrix(gsu: &mut Gsu, rx: u8, ry: u8, rz: u8) -> [[i16; 3]; 3] {
    write_word(gsu, 0x20, (u16::from(rx) << 8) as i16);
    write_word(gsu, 0x22, (u16::from(ry) << 8) as i16);
    write_word(gsu, 0x24, (u16::from(rz) << 8) as i16);
    gsu.run(1, 0x9191);
    let mut result = [[0; 3]; 3];
    for (index, value) in result.iter_mut().flatten().enumerate() {
        *value = read_word(gsu, 0xE4 + index * 2);
    }
    result
}

#[test]
fn q15_zxy_matrix_and_point_rotation_match_sf2_gsu() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };
    let mut gsu = Gsu::new(rom);
    let mut state = 0xC0FF_EE11u32;
    for index in 0..1024 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let rx = state as u8;
        let ry = (state >> 8) as u8;
        let rz = (state >> 16) as u8;
        let oracle = oracle_matrix(&mut gsu, rx, ry, rz);
        let rust = zxy_matrix_q15(rx, ry, rz);
        assert_eq!(
            rust, oracle,
            "matrix case {index}: ({rx:02X},{ry:02X},{rz:02X})"
        );

        let x = state as i16;
        let y = (state.rotate_left(11)) as i16;
        let z = (state.rotate_left(23)) as i16;
        write_word(&mut gsu, 0x68, x);
        write_word(&mut gsu, 0x2C, y);
        write_word(&mut gsu, 0x2E, z);
        gsu.run(1, 0x913A);
        let point = (
            read_word(&gsu, 0x26),
            read_word(&gsu, 0x28),
            read_word(&gsu, 0x2A),
        );
        assert_eq!(
            matrix_rotate_q15(rust, x, y, z),
            point,
            "point case {index}"
        );
    }
}
