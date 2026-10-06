//! Execute source-built SF1 weapon routines, including their initializer tails.
//! Expected state comes from the original instructions, never a native trace.

use std::collections::HashMap;

use sf_game::{alien::Alien, Game};
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::enemy_a::{flatmiss_istrat, relflatmiss_istrat};

// Keep clear of the call harness's bootstrap and return trap.
const OBJECT: u32 = 0x0100;
const WRAM: u32 = 0x7E_0000;

struct Source {
    rom: Vec<u8>,
    symbols: HashMap<String, u32>,
}

impl Source {
    fn load() -> Self {
        Self {
            rom: load_built_rom().expect("weapon oracle requires source-built sf.sfc"),
            symbols: load_symbols(),
        }
    }

    fn symbol(&self, name: &str) -> u32 {
        *self
            .symbols
            .get(name)
            .unwrap_or_else(|| panic!("missing symbol {name}"))
    }

    fn byte(&self, bus: &mut SnesBus, base: u32, name: &str, value: u8) {
        bus.write8(WRAM | (base + self.symbol(name)), value);
    }

    fn word(&self, bus: &mut SnesBus, base: u32, name: &str, value: i16) {
        let address = WRAM | (base + self.symbol(name));
        let [lo, hi] = value.to_le_bytes();
        bus.write8(address, lo);
        bus.write8(address + 1, hi);
    }

    fn seed(&self, bus: &mut SnesBus, object: &Alien, game: &Game) {
        for (name, value) in [
            ("AL_ROTX", object.rotx),
            ("AL_ROTY", object.roty),
            ("AL_ROTZ", object.rotz),
            ("AL_VEL", object.vel),
            ("AL_COUNT", object.count),
            ("AL_COUNT1", object.count1),
            ("AL_HP", object.hp),
            ("AL_AP", object.ap),
            ("AL_SFLAGS", object.sflags),
            ("AL_SFLAGS2", object.sflags2),
            ("AL_SFLAGS3", object.sflags3),
            ("AL_SFLAGS4", object.sflags4),
            ("AL_SBYTE1", object.sbyte1),
            ("AL_SBYTE2", object.sbyte2),
            ("AL_SBYTE3", object.sbyte3),
            ("AL_TYPE", object.type_),
            ("AL_COLLFLAGS", object.collflags),
        ] {
            self.byte(bus, OBJECT, name, value);
        }
        for (name, value) in [
            ("AL_WORLDX", object.worldx),
            ("AL_WORLDY", object.worldy),
            ("AL_WORLDZ", object.worldz),
            ("AL_VX", object.vx),
            ("AL_VY", object.vy),
            ("AL_VZ", object.vz),
        ] {
            self.word(bus, OBJECT, name, value);
        }
        for (name, value) in [
            ("PVIEWVELZ", game.vars.pviewvelz),
            ("OUTVX", game.vars.strategy.view_pitch),
            ("OUTVY", game.vars.strategy.view_yaw),
            ("PLAYER_TURNROT", game.vars.strategy.player_turn_rotation),
        ] {
            self.word(bus, 0, name, value);
        }
    }

    fn run(&self, bus: &mut SnesBus, name: &str) {
        let exit = call(
            bus,
            self.symbol(name),
            &Entry {
                x: OBJECT as u16,
                p: 0x20,
                dbr: 0x7E,
                ..Default::default()
            },
        );
        assert_eq!(
            exit.x, OBJECT as u16,
            "{name} must preserve the current object"
        );
    }

    fn compare(&self, bus: &mut SnesBus, actual: &Alien, context: &str) {
        for (name, value) in [
            ("AL_ROTX", actual.rotx),
            ("AL_ROTY", actual.roty),
            ("AL_ROTZ", actual.rotz),
            ("AL_VEL", actual.vel),
            ("AL_COUNT", actual.count),
            ("AL_COUNT1", actual.count1),
            ("AL_HP", actual.hp),
            ("AL_AP", actual.ap),
            ("AL_SFLAGS", actual.sflags),
            ("AL_SFLAGS2", actual.sflags2),
            ("AL_SFLAGS3", actual.sflags3),
            ("AL_SFLAGS4", actual.sflags4),
            ("AL_SBYTE1", actual.sbyte1),
            ("AL_SBYTE2", actual.sbyte2),
            ("AL_SBYTE3", actual.sbyte3),
            ("AL_TYPE", actual.type_),
            ("AL_COLLFLAGS", actual.collflags),
        ] {
            let expected = bus.read8(WRAM | (OBJECT + self.symbol(name)));
            assert_eq!(value, expected, "{context}: {name}");
        }
        for (name, value) in [
            ("AL_WORLDX", actual.worldx),
            ("AL_WORLDY", actual.worldy),
            ("AL_WORLDZ", actual.worldz),
            ("AL_VX", actual.vx),
            ("AL_VY", actual.vy),
            ("AL_VZ", actual.vz),
        ] {
            let address = WRAM | (OBJECT + self.symbol(name));
            let expected = i16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
            assert_eq!(value, expected, "{context}: {name}");
        }
    }
}

#[test]
fn flat_missile_initializer_includes_the_first_movement_and_lifetime_visit() {
    let source = Source::load();
    for (name, init) in [
        (
            "RELFLATMISS_ISTRAT",
            relflatmiss_istrat as fn(&mut Game, u16),
        ),
        ("FLATMISS_ISTRAT", flatmiss_istrat),
    ] {
        for count in [0, 1, 2, 100, 255] {
            for (pitch, yaw, speed) in [(0, 0, 80), (37, 213, 70), (191, 66, 255)] {
                let mut game = Game::new();
                let id = game.objs.alloc().unwrap();
                game.vars.pviewvelz = 65;
                game.vars.strategy.view_pitch = -1537;
                game.vars.strategy.view_yaw = 1025;
                game.vars.strategy.player_turn_rotation = 2305;
                let object = &mut game.objs.aliens[id as usize];
                object.hp = 9;
                object.count = count;
                object.count1 = 83;
                [object.rotx, object.roty, object.rotz] = [pitch, yaw, 67];
                object.vel = speed;
                [object.sbyte1, object.sbyte2, object.sbyte3] = [99, 21, 61];
                [object.worldx, object.worldy, object.worldz] = [32760, -32760, 32763];
                [object.vx, object.vy, object.vz] = [1234, -999, 2345];
                let object = *object;
                let mut bus = SnesBus::new(source.rom.clone());
                source.seed(&mut bus, &object, &game);
                source.run(&mut bus, name);
                init(&mut game, id);
                source.compare(
                    &mut bus,
                    &game.objs.aliens[id as usize],
                    &format!("{name} count={count} aim=({pitch},{yaw}) speed={speed}"),
                );
                assert_eq!(game.objs.aldead, 0, "lifetime death is deferred");
                assert!(game.objs.aliens[id as usize].stratptr.is_some());
            }
        }
    }
}
