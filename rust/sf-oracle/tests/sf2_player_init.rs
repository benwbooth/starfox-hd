//! Differential proof for the two SF2 player-selection initializers.

use sf2_game::object::*;
use sf2_game::oracle_compat::Game;
use sf_oracle::{call, Entry, SnesBus};

fn retail_sf2() -> Option<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)?;
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc")).ok()
}

fn seeded_game(rom: Vec<u8>, initializer: u32) -> (Game, u16) {
    let mut game = Game::new(rom).unwrap();
    let object = allocate(&mut game.memory, 0).unwrap();
    game.memory
        .write_word(object + FIELD_STRATEGY, initializer as u16);
    game.memory
        .write_byte(object + FIELD_STRATEGY + 2, (initializer >> 16) as u8);

    // Exercise flag preservation, the signed-byte minimum update, auxiliary
    // target flags, and the overlapping three-byte `$D816..$D818` copy.
    game.memory.write_byte(object + 0x09, 0xFF);
    game.memory.write_byte(object + 0x20, 0x41);
    game.memory.write_byte(object + 0x21, 0xA4);
    game.memory.write_byte(object + 0x22, 0x83);
    game.memory.write_byte(object + 0x23, 0x9A);
    game.memory.write_byte(object + 0x24, 0x51);
    game.memory.write_byte(object + 0x26, 0x22);
    game.memory.write_byte(0x1AA6, 0);
    game.memory.write_byte(0x1DD1, 0x35);
    game.memory.write_byte(0x1DD5, 0xF2);
    game.memory.write_byte(0x1E14, 0x67);
    game.memory.write_byte(0xD816, 0x12);
    game.memory.write_byte(0xD817, 0x34);
    game.memory.write_byte(0xD818, 0x56);
    (game, object)
}

#[test]
fn player_initializers_match_retail_65816_state() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for initializer in [0x0682F9u32, 0x0682ED] {
        let (mut rust, object) = seeded_game(rom.clone(), initializer);
        let mut oracle = SnesBus::new(rom.clone());
        for (index, byte) in rust.memory.main_state().iter().copied().enumerate() {
            oracle.write8(0x7E_0000 + index as u32, byte);
        }
        call(
            &mut oracle,
            initializer,
            &Entry {
                x: object,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        rust.initialize_player_strategy(object).unwrap();

        // The oracle bootstrap owns low WRAM below the first object. Everything
        // from the retail object pool through the end of bank `$7E` is game
        // state and must be byte-identical after the call.
        let differences: Vec<_> = (OBJECT_POOL_BASE..=u16::MAX)
            .filter_map(|address| {
                let actual = rust.memory.read_byte(address);
                let expected = oracle.read8(0x7E_0000 + u32::from(address));
                (actual != expected).then_some((address, actual, expected))
            })
            .collect();
        assert!(
            differences.is_empty(),
            "initializer=${initializer:06X} differences={:?}",
            &differences[..differences.len().min(80)]
        );
    }
}

#[test]
fn player_first_entry_matches_retail_before_main_tick() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for initializer in [0x0682F9u32, 0x0682ED] {
        let (mut rust, object) = seeded_game(rom.clone(), initializer);
        rust.initialize_player_strategy(object).unwrap();

        // `$06:845C` tail-jumps directly into the normal `$06:9C27` player
        // tick. Replace only that first opcode with RTL in the oracle copy so
        // the test observes the complete one-shot entry routine at its exact
        // boundary, without executing an unrelated gameplay frame.
        let mut trapped_rom = rom.clone();
        trapped_rom[0x031C27] = 0x6B;
        let mut oracle = SnesBus::new(trapped_rom);

        // Cover nonzero character-table selection and the alternate shape
        // pair selected by `$1DE3.7`, plus preservation of unrelated bits.
        rust.memory.write_byte(0x1DE2, 4);
        rust.memory.write_byte(0x1DE3, 0x80);
        rust.memory.write_byte(0x0360, 0x49);
        for (index, byte) in rust.memory.main_state().iter().copied().enumerate() {
            oracle.write8(0x7E_0000 + index as u32, byte);
        }

        call(
            &mut oracle,
            0x06845C,
            &Entry {
                x: object,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        rust.enter_player_main_strategy(object).unwrap();

        let differences: Vec<_> = (OBJECT_POOL_BASE..=u16::MAX)
            .filter_map(|address| {
                let actual = rust.memory.read_byte(address);
                let expected = oracle.read8(0x7E_0000 + u32::from(address));
                (actual != expected).then_some((address, actual, expected))
            })
            .collect();
        assert!(
            differences.is_empty(),
            "initializer=${initializer:06X} differences={:?}",
            &differences[..differences.len().min(100)]
        );
    }
}

#[test]
fn player_main_frames_match_retail_65816_and_gsu() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    let (mut rust, object) = seeded_game(rom.clone(), 0x0682F9);
    rust.initialize_player_strategy(object).unwrap();
    rust.enter_player_main_strategy(object).unwrap();

    let mut oracle = SnesBus::new(rom);
    oracle.enable_gsu();
    for (index, byte) in rust.memory.main_state().iter().copied().enumerate() {
        oracle.write8(0x7E_0000 + index as u32, byte);
    }

    let inputs = [
        0,
        sf_core::pad::RIGHT | sf_core::pad::B,
        sf_core::pad::UP | sf_core::pad::X,
        sf_core::pad::LEFT | sf_core::pad::A,
    ];
    let mut previous = 0;
    for (frame, pad) in inputs.into_iter().enumerate() {
        let trigger = pad & !previous;
        previous = pad;
        rust.memory.write_word(0x1936, pad);
        rust.memory.write_word(0x1938, trigger);
        oracle.write16(0x7E_1936, pad);
        oracle.write16(0x7E_1938, trigger);

        call(
            &mut oracle,
            0x069C27,
            &Entry {
                x: object,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        rust.tick_player_main_strategy(object).unwrap();

        let differences: Vec<_> = (OBJECT_POOL_BASE..=u16::MAX)
            .filter_map(|address| {
                let actual = rust.memory.read_byte(address);
                let expected = oracle.read8(0x7E_0000 + u32::from(address));
                (actual != expected).then_some((address, actual, expected))
            })
            .collect();
        assert!(
            differences.is_empty(),
            "frame={frame} pad=${pad:04X} differences={:?}",
            &differences[..differences.len().min(100)]
        );
    }
}
