//! Differential proof for SF2 path opcode `$035`'s shared weapon formatter.

use sf2_game::object::*;
use sf2_game::oracle_compat::Game;
use sf2_path::Sf2PathHost;
use sf_oracle::{call, Entry, SnesBus};

fn retail_sf2() -> Option<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)?;
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc")).ok()
}

fn seeded_game(rom: Vec<u8>, selector: u8) -> (Game, u16) {
    let mut game = Game::new(rom).unwrap();
    let source = allocate(&mut game.memory, 0).unwrap();
    let player = allocate(&mut game.memory, source).unwrap();
    game.memory.write_word(CURRENT_OBJECT, source);
    game.memory.write_word(PLAYER_ONE, player);
    game.memory.write_word(player + FIELD_PATH, 6);
    game.memory.write_byte(0x6AA0 + 6, 0);
    for (field, value) in [
        (FIELD_X, (-1234i16) as u16),
        (FIELD_Y, 2345),
        (FIELD_Z, (-3456i16) as u16),
    ] {
        game.memory.write_word(source + field, value);
    }
    for (field, value) in [
        (FIELD_ROT_X, 0x13),
        (FIELD_ROT_Y, 0x29),
        (FIELD_ROT_Z, 0xB7),
    ] {
        game.memory.write_byte(source + field, value);
    }
    game.memory.write_byte(source + 0x18, 0x25);
    game.memory.write_byte(source + 0x2F, selector);
    for (address, value) in [(0xE0, 0x11), (0xE1, 0x28), (0xE2, 0xE9), (0xE3, 0x9B)] {
        game.memory.write_byte(address, value);
    }
    (game, source)
}

#[test]
fn reachable_weapon_selectors_match_retail_65816() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for selector in [0x12, 0x14, 0x16, 0x1A, 0x1E] {
        let (initial, source) = seeded_game(rom.clone(), selector);
        let mut oracle = SnesBus::new(rom.clone());
        for (index, byte) in initial.memory.main_state().iter().copied().enumerate() {
            oracle.write8(0x7E_0000 + index as u32, byte);
        }

        // `$7F:88C4` is a near RTS routine. Install a four-byte JSR/RTL
        // wrapper in unused bank-$7F WRAM so the standard far-call harness
        // can observe its completed return unambiguously.
        for (index, byte) in [0x20, 0xC4, 0x88, 0x6B].into_iter().enumerate() {
            oracle.write8(0x7F_D100 + index as u32, byte);
        }
        call(
            &mut oracle,
            0x7F_D100,
            &Entry {
                x: source,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        let oracle_weapon = oracle.wram_read16(0xD771);
        if object_index(oracle_weapon).is_some() {
            let flags = oracle.read8(0x7E_0000 + u32::from(oracle_weapon + 0x31)) | 0x10;
            oracle.write8(0x7E_0000 + u32::from(oracle_weapon + 0x31), flags);
        }

        let (mut rust, _) = seeded_game(rom.clone(), selector);
        Sf2PathHost::fire_weapon(&mut rust).unwrap();
        let rust_weapon = rust.memory.read_word(0xD771);
        assert_eq!(
            rust_weapon, oracle_weapon,
            "selector ${selector:02X} object"
        );

        for offset in 0x04..OBJECT_STRIDE {
            assert_eq!(
                rust.memory.read_byte(rust_weapon + offset),
                oracle.read8(0x7E_0000 + u32::from(oracle_weapon + offset)),
                "selector ${selector:02X} object field +${offset:02X}",
            );
        }
        for offset in 0..OBJECT_STRIDE {
            assert_eq!(
                rust.memory
                    .read_byte(rust_weapon.wrapping_add(0x1CC1).wrapping_add(offset)),
                oracle.read8(
                    0x7E_0000 + u32::from(oracle_weapon.wrapping_add(0x1CC1).wrapping_add(offset)),
                ),
                "selector ${selector:02X} extension +${offset:02X}",
            );
        }
        for address in [0x1D69u16, 0x1D6B, 0xD771] {
            assert_eq!(
                rust.memory.read_byte(address),
                oracle.read8(0x7E_0000 + u32::from(address)),
                "selector ${selector:02X} global ${address:04X}",
            );
        }
        assert_eq!(
            rust.memory.read_word(source + 0x1C),
            oracle.wram_read16(u32::from(source + 0x1C)),
            "selector ${selector:02X} source cross-link",
        );
    }
}
