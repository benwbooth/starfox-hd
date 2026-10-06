//! Compare the native missile boundary branches with original SF1 instructions.
//! In particular, MORE/LESS test the sign of a wrapping word subtraction, not
//! an unbounded host-language comparison; vertical equality is outside bounds.

use sf_game::Game;
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::missbound_chk_exp;

const OBJECT: u16 = 0x0100;
const PLAYER: u16 = 0x0500;
const START_HP: u8 = 7;
const START_FLAGS: u8 = 0x54;

#[test]
fn every_boundary_flag_combination_matches_original_edge_and_player_gates() {
    let symbols = load_symbols();
    let mut bus = SnesBus::new(load_built_rom().expect("source-built SF1 ROM required"));
    let symbol = |name: &str| {
        *symbols
            .get(name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let word = |bus: &mut SnesBus, base: u16, name: &str, value: i16| {
        bus.write16(u32::from(base) + symbol(name), value as u16);
    };
    let byte = |bus: &mut SnesBus, base: u16, name: &str, value: u8| {
        bus.write8(u32::from(base) + symbol(name), value);
    };
    let mut game = Game::new();
    let decoy = game.objs.alloc().unwrap();
    let player = game.objs.alloc().unwrap();
    let missile = game.objs.alloc().unwrap();
    // playpt is a relationship, not the first pool slot. The decoy deliberately
    // lies beyond the opposite gate so a hard-coded slot-zero lookup fails.
    game.vars.player_object = player as i16;
    game.objs.aliens[decoy as usize].worldx = i16::MAX;
    game.vars.minpmove_y = -100;
    for (name, field, value) in [
        ("MINMMOVEX", sv::MINMMOVEX, -200),
        ("MAXMMOVEX", sv::MAXMMOVEX, 200),
        ("MAXMMOVEY", sv::MAXMMOVEY, 100),
        ("MISSBTOPLEFT", sv::MISSBTOPLEFT, -100),
        ("MISSBTOPRIGHT", sv::MISSBTOPRIGHT, 100),
        ("MISSBBOTLEFT", sv::MISSBBOTLEFT, 0),
    ] {
        word(&mut bus, 0, name, value);
        game.vars.set_sv_i16(field, value);
    }
    word(&mut bus, 0, "MINPMOVEY", -100);
    word(&mut bus, 0, "PLAYPT", PLAYER as i16);
    word(&mut bus, 0, "AL1PT", OBJECT as i16);
    let inputs = [-201, -200, -199, 0, 199, 200, 201, i16::MIN, i16::MAX];
    let vertical = [-101, -100, -99, 0, 99, 100, 101, i16::MIN, i16::MAX];
    let player_positions = [-101, -100, -99, -1, 0, 1, 99, 100, 101, i16::MIN, i16::MAX];
    let mut checked = 0;
    for flags in u8::MIN..=u8::MAX {
        game.vars.set_sv_u8(sv::MISSBOUNDFLAGS, flags);
        byte(&mut bus, 0, "MISSBOUNDFLAGS", flags);
        for x in inputs {
            for y in vertical {
                for player_x in player_positions {
                    game.objs.aliens[player as usize].worldx = player_x;
                    let object = &mut game.objs.aliens[missile as usize];
                    [object.worldx, object.worldy] = [x, y];
                    object.hp = START_HP;
                    object.sflags2 = START_FLAGS;
                    word(&mut bus, PLAYER, "AL_WORLDX", player_x);
                    word(&mut bus, OBJECT, "AL_WORLDX", x);
                    word(&mut bus, OBJECT, "AL_WORLDY", y);
                    byte(&mut bus, OBJECT, "AL_HP", START_HP);
                    byte(&mut bus, OBJECT, "AL_SFLAGS2", START_FLAGS);
                    let result = call(
                        &mut bus,
                        symbol("MISSBOUNDCHKEXP"),
                        &Entry {
                            x: OBJECT,
                            p: 0x20,
                            ..Default::default()
                        },
                    );
                    assert!(result.returned, "source boundary routine must return");
                    missbound_chk_exp(&mut game, missile);
                    let actual = &game.objs.aliens[missile as usize];
                    assert_eq!(
                        [actual.hp, actual.sflags2],
                        [
                            bus.read8(u32::from(OBJECT) + symbol("AL_HP")),
                            bus.read8(u32::from(OBJECT) + symbol("AL_SFLAGS2"))
                        ],
                        "flags={flags:#04x} missile=({x},{y}) player_x={player_x}"
                    );
                    assert!(actual.active);
                    assert_eq!(game.objs.aldead, 0);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 228096);
}
