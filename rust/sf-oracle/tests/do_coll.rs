//! Execute the original damage routine against the SHIPPING implementation.
//! A wrapped subtraction's sign is not an unsigned saturating subtraction.

use sf_game::{vars::PSF3_INTUNNEL, Game};
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const OBJECT: u32 = 256;

#[test]
fn production_damage_matches_source_for_all_health_and_attack_bytes() {
    let symbols = load_symbols();
    let mut bus = SnesBus::new(load_built_rom().expect("damage oracle requires source-built ROM"));
    let mut game = Game::new();
    let victim = game.objs.alloc().unwrap();
    let entry = Entry {
        x: OBJECT as u16,
        p: 0x20,
        ..Default::default()
    };
    let damage_entry = symbols["DO_COLL_L"];
    let health = OBJECT + symbols["AL_HP"];
    let cooldown = OBJECT + symbols["AL_COLLCOUNT"];
    let power = symbols["X1"];
    let period = symbols["TPA"];
    let ship_flags = symbols["PSHIPFLAGS3"];
    for tunnel in [false, true] {
        for hp in 0..=255u8 {
            for ap in 0..=255u8 {
                // Exhaust the damage boundary; the separate test below
                // exhausts all cooldown values without duplicating 65K pairs.
                game.objs.aliens[victim as usize].hp = hp;
                game.objs.aliens[victim as usize].collcount = 1;
                game.vars.pshipflags3 = if tunnel { PSF3_INTUNNEL } else { 0 };
                bus.write8(health, hp);
                bus.write8(cooldown, 1);
                bus.write8(power, ap);
                bus.write8(period, sf_game::vars::FRAMESPERAP);
                bus.write8(ship_flags, game.vars.pshipflags3);
                let exit = call(&mut bus, damage_entry, &entry);
                assert!(exit.returned, "original damage did not return");
                game.coldet_apply_damage(victim, ap, 0);
                let actual = &game.objs.aliens[victim as usize];
                assert_eq!(
                    (actual.hp, actual.collcount),
                    (bus.read8(health), bus.read8(cooldown)),
                    "hp={hp} ap={ap} tunnel={tunnel}"
                );
            }
        }
    }
}

#[test]
fn production_damage_matches_every_source_cooldown_byte() {
    let symbols = load_symbols();
    let mut bus = SnesBus::new(load_built_rom().expect("damage oracle requires source-built ROM"));
    let mut game = Game::new();
    let victim = game.objs.alloc().unwrap();
    for cc in 0..=255u8 {
        for hp in [0, 1, 4, 127, 128, 255] {
            game.objs.aliens[victim as usize].hp = hp;
            game.objs.aliens[victim as usize].collcount = cc;
            bus.write8(OBJECT + symbols["AL_HP"], hp);
            bus.write8(OBJECT + symbols["AL_COLLCOUNT"], cc);
            bus.write8(symbols["X1"], 8);
            bus.write8(symbols["TPA"], sf_game::vars::FRAMESPERAP);
            assert!(
                call(
                    &mut bus,
                    symbols["DO_COLL_L"],
                    &Entry {
                        x: OBJECT as u16,
                        p: 0x20,
                        ..Default::default()
                    }
                )
                .returned
            );
            game.coldet_apply_damage(victim, 8, 0);
            let actual = &game.objs.aliens[victim as usize];
            assert_eq!(
                (actual.hp, actual.collcount),
                (
                    bus.read8(OBJECT + symbols["AL_HP"]),
                    bus.read8(OBJECT + symbols["AL_COLLCOUNT"])
                ),
                "hp={hp} cooldown={cc}"
            );
        }
    }
}
