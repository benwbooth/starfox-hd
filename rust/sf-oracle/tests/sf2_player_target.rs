//! Differential proof for SF2 path opcodes `$0C6/$0C7`'s player-target update.

use sf2_game::object::*;
use sf2_game::oracle_compat::Game;
use sf2_path::{PlayerTargetUpdate, Sf2PathHost};
use sf_oracle::{call, Entry, SnesBus};

fn retail_sf2() -> Option<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)?;
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc")).ok()
}

fn seeded_game(
    rom: Vec<u8>,
    target_position: [i16; 3],
    anchor_position: [i16; 3],
) -> (Game, u16, u16) {
    let mut game = Game::new(rom).unwrap();
    let target = allocate(&mut game.memory, 0).unwrap();
    let player = allocate(&mut game.memory, target).unwrap();
    let linked = allocate(&mut game.memory, player).unwrap();
    game.memory.write_word(CURRENT_OBJECT, target);
    game.memory.write_word(PLAYER_ONE, player);
    game.memory.write_word(player + FIELD_PATH, 6);
    game.memory.write_word(target + 0x1CE6, linked);
    game.memory.write_byte(linked + 0x12, 0x41);

    for ((field, target_value), anchor_value) in [FIELD_X, FIELD_Y, FIELD_Z]
        .into_iter()
        .zip(target_position)
        .zip(anchor_position)
    {
        game.memory.write_word(target + field, target_value as u16);
        game.memory.write_word(0x033F + field, anchor_value as u16);
    }
    game.memory.write_word(0x033F + FIELD_ROT_X, 0x0571);
    game.memory.write_word(0x033F + FIELD_ROT_Y, 0x0B29);

    game.memory.write_word(0x6BBC + 6, 0xFFFF);
    game.memory.write_word(0x6BBA + 6, 0x4567);
    game.memory.write_byte(0x6BB6 + 6, 0xD3);
    game.memory.write_byte(0x6BC2 + 6, 0x01);
    game.memory.write_word(0x6BCA + 6, 0);
    game.memory.write_byte(0x1DBC, 0xA5);
    game.memory.write_byte(0x5E, 0xFF);
    (game, target, linked)
}

#[test]
fn player_target_updates_match_retail_65816() {
    let Some(rom) = retail_sf2() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };
    let positions = [
        ([-1234, 2345, -3456], [321, -789, 1444]),
        ([4100, -822, 731], [-900, 2200, -2700]),
        ([-17, 95, 44], [711, 100, 995]),
        ([16300, -12000, -9000], [-15200, 13000, 15100]),
    ];

    for &(target_position, anchor_position) in &positions {
        for scenario in 0..4u8 {
            for update in [PlayerTargetUpdate::FlagLinked, PlayerTargetUpdate::Flag08] {
                let (mut initial, target, linked) =
                    seeded_game(rom.clone(), target_position, anchor_position);
                match scenario {
                    1 => initial.memory.write_byte(0x6BC2 + 6, 0x11),
                    2 => initial.memory.write_word(0x6BBC + 6, 1),
                    3 => {
                        initial.memory.write_word(0x6BBC + 6, 1);
                        initial.memory.write_word(0x6BCA + 6, target);
                    }
                    _ => {}
                }
                let mut oracle = SnesBus::new(rom.clone());
                for (index, byte) in initial.memory.main_state().iter().copied().enumerate() {
                    oracle.write8(0x7E_0000 + index as u32, byte);
                }

                // The small path-handler wrapper clears these control bits before
                // calling the far routine.  Call the reusable retail leaf directly
                // so the path VM tail-jump cannot consume the test harness return.
                oracle.write8(0x7E_005E, 0xE7);
                let routine = match update {
                    PlayerTargetUpdate::FlagLinked => 0x07_B1FD,
                    PlayerTargetUpdate::Flag08 => 0x07_B1EA,
                };
                call(
                    &mut oracle,
                    routine,
                    &Entry {
                        x: target,
                        p: 0x20,
                        ..Default::default()
                    },
                );
                if update == PlayerTargetUpdate::FlagLinked {
                    let value = oracle.read8(0x7E_0000 + u32::from(linked + 0x12)) | 0x20;
                    oracle.write8(0x7E_0000 + u32::from(linked + 0x12), value);
                }

                let mut rust = initial;
                Sf2PathHost::update_player_target(&mut rust, update).unwrap();

                let mut addresses = vec![0x005Eu16, 0x0079, 0x007A, 0x007B, 0x007C, 0x12DE, 0x12DF];
                addresses.extend(0x1DAE..=0x1DC1);
                addresses.extend(0x6BAD + 6..=0x6BD1 + 6);
                addresses.push(linked + 0x12);
                for address in addresses {
                    assert_eq!(
                    rust.memory.read_byte(address),
                    oracle.read8(0x7E_0000 + u32::from(address)),
                        "scenario={scenario} update={update:?} target={target_position:?} anchor={anchor_position:?} address=${address:04X}",
                );
                }
            }
        }
    }
}
