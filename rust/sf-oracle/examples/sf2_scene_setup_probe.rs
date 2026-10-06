//! Observe the original opening loader's scene and display publications.
//! Addresses are confined to this verification tool, never native game state.

use sf_oracle::RetailMachine;

fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rom = std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc")).unwrap();
    let mut source = RetailMachine::new(rom);
    for address in [
        0x0DBCCF, 0x03C80B, 0x03D605, 0x7F0BBF, 0x7F0C24, 0x03C813, 0x03D52C, 0x03D56F,
    ] {
        assert!(source.tick_until_cpu_execution(0, address, 240).unwrap());
        println!("boundary={address:06X}");
        for field in [
            0x129B, 0x15FD, 0x15FF, 0x1603, 0x162A, 0x162C, 0x1812, 0x181C, 0x18A7, 0x18A9, 0x18AA,
            0x18CC, 0x18D2, 0x18E7, 0x1A88, 0x1A89, 0x1A8A, 0x1AA6, 0x1AA7, 0x1B3C, 0x1C51, 0x1C52,
            0x1C56, 0x1D1F, 0x1D22, 0x1D4B, 0x1D67,
        ] {
            print!("{field:04X}={:04X} ", source.peek16(0x7E0000 + field));
        }
        println!();
        for field in [0x4E, 0x50, 0x1B2, 0x1BA] {
            let word = u16::from(source.peek_gsu_ram(field))
                | u16::from(source.peek_gsu_ram(field + 1)) << 8;
            print!("gsu:{field:04X}={word:04X} ");
        }
        let video = source.ppu_frame();
        println!(
            "display={:02X?} vertical={:?}",
            &video.registers[..48],
            video.bg_vofs
        );
    }
}
