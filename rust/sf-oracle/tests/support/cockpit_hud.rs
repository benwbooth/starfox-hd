//! CPU-side enable/roll ownership and source ship/damage publication.
use super::{
    repair_chain::{scene, seed},
    Source, OBJECT, TARGET, WRAM,
};
use sf_core::player_view::PlayerViewMode;
use sf_core::shape::{resolve_shape_word, SF1_SHAPE_COCKPIT_PLAYER};
use sf_oracle::{call, call_near, Entry};
use sf_strat::player::{player_move_init, select_ship, strat_player};

const MODES: [PlayerViewMode; 5] = [
    PlayerViewMode::Exterior,
    PlayerViewMode::CloseExterior,
    PlayerViewMode::EnteringCockpit,
    PlayerViewMode::Cockpit,
    PlayerViewMode::LeavingCockpit,
];

#[test]
fn enable_publication_matches_original_and_preserves_every_roll_byte() {
    let source = Source::load();
    for mode in MODES {
        for roll in 0..=u8::MAX {
            let (mut game, _, id, _) = scene();
            game.vars.player_view_mode = mode;
            game.vars.strategy.hud_rotation = i16::from_le_bytes([roll, roll ^ 0x5A]);
            let mut bus = seed(&source, &game, id);
            source.word(&mut bus, 0, "INTERNALPLAYPT", OBJECT as i16);
            source.word(&mut bus, 0, "DUMMYOBJ", 0x0C00);
            source.word(&mut bus, 0, "HUDROT", game.vars.strategy.hud_rotation);
            source.byte(&mut bus, 0, "SPLAYERFLYMODE", mode as u8);
            let exit = call(
                &mut bus,
                source.symbol("INIT_STRATS_L"),
                &Entry {
                    p: 0x20,
                    dbr: 0x7E,
                    ..Default::default()
                },
            );
            assert!(exit.returned);
            game.run_strategies();
            let address = WRAM | source.symbol("HUDROT");
            assert_eq!(
                game.vars.strategy.hud_rotation.to_le_bytes(),
                [bus.read8(address), bus.read8(address + 1)],
                "mode={mode:?} roll={roll}"
            );
        }
    }
}

#[test]
fn selected_shape_and_damage_match_original_for_every_ship_flag_and_view() {
    let source = Source::load();
    for mode in MODES {
        for ship in 0..7 {
            for flags in 0..=u8::MAX {
                let (mut game, player, id, _) = scene();
                game.vars.player_view_mode = mode;
                game.vars.pshipflags = flags;
                select_ship(&mut game, ship);
                let mut bus = seed(&source, &game, id);
                source.byte(&mut bus, 0, "SPLAYERFLYMODE", mode as u8);
                let table = source.symbol("PLAYER_SHAPES") + u32::from(ship) * 8;
                for (index, name) in [
                    "PLAYERSHAPE",
                    "PLAYERSHAPEL",
                    "PLAYERSHAPER",
                    "PLAYERSHAPELR",
                ]
                .into_iter()
                .enumerate()
                {
                    let address = table + index as u32 * 2;
                    let shape = u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
                    source.word(&mut bus, 0, name, shape as i16);
                }
                let exit = call_near(
                    &mut bus,
                    source.symbol("SETCURRPSHAPE"),
                    &Entry {
                        x: TARGET as u16,
                        p: 0x20,
                        dbr: 0x7E,
                        ..Default::default()
                    },
                );
                assert!(exit.returned);
                // The native movement initializer owns this same publication.
                player_move_init(&mut game, player);
                let address = WRAM | (TARGET + source.symbol("AL_SHAPE"));
                let shape = u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
                assert_eq!(
                    game.objs.aliens[player as usize].shape,
                    resolve_shape_word(shape),
                    "mode={mode:?} ship={ship} flags={flags}"
                );
                assert_eq!(
                    game.vars.strategy.cockpit_hud_left_wing_broken,
                    bus.read8(source.symbol("M_HUDFLAGS")) & 1 != 0
                );
                assert_eq!(
                    game.vars.strategy.cockpit_hud_right_wing_broken,
                    bus.read8(source.symbol("M_HUDFLAGS")) & 2 != 0
                );
                if mode == PlayerViewMode::Cockpit {
                    assert_eq!(
                        game.objs.aliens[player as usize].shape,
                        SF1_SHAPE_COCKPIT_PLAYER
                    );
                }
            }
        }
    }
}

#[test]
fn movement_publishes_roll_without_changing_the_inherited_enable_byte() {
    let source = Source::load();
    for roll in 0..=u8::MAX {
        for high in [0, 1, 127, 255] {
            let (mut game, player, id, _) = scene();
            select_ship(&mut game, 0);
            game.objs.aliens[player as usize].hp = 40;
            game.vars.strategy.hud_rotation = i16::from_le_bytes([roll ^ 0xA5, high]);
            game.vars.strategy.player_depth_strategy_offset = roll;
            let mut bus = seed(&source, &game, id);
            source.byte(&mut bus, TARGET, "AL_HP", 40);
            source.byte(&mut bus, 0, "PLAYER_ZSTRATADD", roll);
            source.word(&mut bus, 0, "HUDROT", game.vars.strategy.hud_rotation);
            let exit = call_near(
                &mut bus,
                source.symbol("PLAYERMOVE_SROU"),
                &Entry {
                    x: TARGET as u16,
                    p: 0x20,
                    dbr: 0x7E,
                    ..Default::default()
                },
            );
            assert!(exit.returned);
            strat_player(&mut game, player);
            let address = WRAM | source.symbol("HUDROT");
            assert_eq!(
                game.vars.strategy.hud_rotation.to_le_bytes(),
                [bus.read8(address), bus.read8(address + 1)],
                "roll={roll} high={high}"
            );
            assert_eq!(bus.read8(address + 1), high);
        }
    }
}
