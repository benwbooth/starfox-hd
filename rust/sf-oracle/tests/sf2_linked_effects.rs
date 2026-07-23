//! Differential proof for SF2 path opcode `$14A`'s linked-object effect pass.

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

fn seeded_game(rom: Vec<u8>) -> (Game, u16, [u16; 2]) {
    let mut game = Game::new(rom).unwrap();
    let current = allocate(&mut game.memory, 0).unwrap();
    let linked_a = allocate(&mut game.memory, current).unwrap();
    let linked_b = allocate(&mut game.memory, linked_a).unwrap();
    let player = allocate(&mut game.memory, linked_b).unwrap();
    game.memory.write_word(CURRENT_OBJECT, current);
    game.memory.write_word(PLAYER_ONE, player);
    game.memory.write_word(PLAYER_TWO, 0);
    game.memory.write_word(player + FIELD_PATH, 6);
    game.memory.write_byte(player + FIELD_ROT_Y, 0x29);
    game.memory.write_byte(0x6AA0 + 6, 0);

    game.memory.write_byte(current + 0x25, 0x10);
    game.memory.write_byte(current + FIELD_ROT_X, 0x17);
    game.memory.write_byte(current + FIELD_ROT_Y, 0x31);
    game.memory.write_byte(current + FIELD_ROT_Z, 0x5B);
    game.memory.write_byte(current + 0x18, 0x24);
    for (field, value) in [(FIELD_X, 1111), (FIELD_Y, 2222), (FIELD_Z, 3333)] {
        game.memory.write_word(current + field, value);
    }

    let nodes = [0x2500u16, 0x2510];
    game.memory.write_word(current + 0x1E, nodes[0]);
    game.memory.write_word(nodes[0], nodes[1]);
    game.memory.write_word(nodes[0] + 4, linked_a);
    game.memory.write_word(nodes[1], 0);
    game.memory.write_word(nodes[1] + 4, linked_b);
    for (index, linked) in [linked_a, linked_b].into_iter().enumerate() {
        game.memory.write_byte(linked + 0x31, 0x0C);
        game.memory.write_byte(linked + 0x21, 0);
        game.memory
            .write_word(linked + FIELD_SHAPE, 0xC123 + index as u16);
        game.memory
            .write_byte(linked + FIELD_ROT_X, 0x21 + index as u8 * 7);
        game.memory
            .write_byte(linked + FIELD_ROT_Y, 0x53 + index as u8 * 9);
        for (field, value) in [
            (FIELD_X, (-800i16 + index as i16 * 1700) as u16),
            (FIELD_Y, 1900 + index as u16 * 1300),
            (FIELD_Z, (-2700i16 + index as i16 * 2100) as u16),
        ] {
            game.memory.write_word(linked + field, value);
        }
        game.memory
            .write_word(linked + 0x1CCD, 0x4200 + index as u16);
    }
    game.memory.write_byte(linked_b + 0x20, 0x20);
    game.memory.write_byte(linked_b + 0x1CC8, 0x37);
    game.memory.write_byte(linked_b + 0x1CDA, 0x49);

    for (address, value) in [(0xE0, 0x11), (0xE1, 0x28), (0xE2, 0xE9), (0xE3, 0x9B)] {
        game.memory.write_byte(address, value);
    }
    game.memory.write_byte(0x5E, 0xFF);
    game.memory.write_byte(0x1AA6, 0x02);
    (game, current, [linked_a, linked_b])
}

#[test]
fn linked_object_effects_match_retail_65816() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };
    let (initial, current, linked) = seeded_game(rom.clone());
    let mut oracle = SnesBus::new(rom.clone());
    for (index, byte) in initial.memory.main_state().iter().copied().enumerate() {
        oracle.write8(0x7E_0000 + index as u32, byte);
    }
    oracle.write8(0x7E_005E, 0xE7);
    call(
        &mut oracle,
        0x07_F1AE,
        &Entry {
            x: current,
            dbr: 0x7E,
            p: 0x20,
            ..Default::default()
        },
    );

    let mut rust = initial;
    Sf2PathHost::spawn_linked_object_effects(&mut rust).unwrap();

    for object in 0..OBJECT_COUNT {
        let base = object_address(object);
        for offset in 0..OBJECT_STRIDE {
            assert_eq!(
                rust.memory.read_byte(base + offset),
                oracle.read8(0x7E_0000 + u32::from(base + offset)),
                "object {object} field +${offset:02X}",
            );
            assert_eq!(
                rust.memory
                    .read_byte(base.wrapping_add(0x1CC1).wrapping_add(offset)),
                oracle
                    .read8(0x7E_0000 + u32::from(base.wrapping_add(0x1CC1).wrapping_add(offset)),),
                "object {object} extension +${offset:02X}",
            );
        }
    }
    for address in [
        ACTIVE_LIST,
        ACTIVE_LIST + 1,
        FREE_LIST,
        FREE_LIST + 1,
        CURRENT_OBJECT,
        CURRENT_OBJECT + 1,
        0x005E,
        0x00E0,
        0x00E1,
        0x00E2,
        0x00E3,
        0x00E4,
        0x14B0,
        0x14B2,
        0x14B4,
        0x14B6,
        0x14B7,
        0x14B8,
        0x14B9,
        0x1D69,
        0x1D6B,
    ] {
        assert_eq!(
            rust.memory.read_byte(address),
            oracle.read8(0x7E_0000 + u32::from(address)),
            "global ${address:04X}",
        );
    }
    for object in linked {
        assert_eq!(
            rust.memory.read_byte(object + 0x21),
            oracle.read8(0x7E_0000 + u32::from(object + 0x21)),
        );
    }
}
