//! Execute each unmodified PSTRATS camera tail from the source-built ROM.
//! Movement finishes before this boundary; the shared depth chase begins at
//! the stop boundary. Neither movement nor whole-camera timing is claimed here.

use sf_core::player_view::PlayerViewMode;
use sf_game::Game;
use sf_oracle::{load_built_rom, load_symbols, SnesBus};
use sf_strat::player_view_anchor::{apply_flight_view_anchor, FlightViewAnchor};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use w65c816::{AddressType, Signals, System, CPU};

const WRAM: u32 = 0x7E0000;
const ACTOR: u32 = 0x0400;
const DEPTH_SENTINEL: i16 = 12345;
const MODES: [PlayerViewMode; 5] = [
    PlayerViewMode::Exterior,
    PlayerViewMode::CloseExterior,
    PlayerViewMode::EnteringCockpit,
    PlayerViewMode::Cockpit,
    PlayerViewMode::LeavingCockpit,
];

struct Original {
    bus: SnesBus,
    reset: bool,
    stop: u32,
    names: HashMap<String, u32>,
}

impl System for Original {
    fn read(&mut self, address: u32, kind: AddressType, signals: &Signals) -> u8 {
        self.bus.read(address, kind, signals)
    }
    fn write(&mut self, address: u32, value: u8, kind: AddressType, signals: &Signals) {
        self.bus.write(address, value, kind, signals);
    }
    fn res(&mut self) -> bool {
        std::mem::take(&mut self.reset)
    }
}

impl Original {
    fn new(strategy: &str, movement: &str, camera_bytes: u32) -> Self {
        let rom = load_built_rom().expect("camera verification needs source-built SF1 ROM");
        assert_eq!(
            format!("{:x}", Sha256::digest(&rom)),
            "fc1a444b269fc577ad2d1e57801f0b70379202f1d55aed259e7f6b491b2d3a72"
        );
        let names = load_symbols();
        let mut bus = SnesBus::new(rom);
        let start = names[strategy];
        let entry = start + 3;
        let stop = entry + camera_bytes;
        for (address, target) in [(start, names[movement]), (stop, names["VIEWMOVE_SROU"])] {
            assert_eq!(bus.read8(address), 0x20, "expected original JSR boundary");
            assert_eq!(bus.read16(address + 1), target as u16);
        }
        // Bootstrap only in scratch RAM: native mode, byte accumulator,
        // word index, original data bank and an object in direct-page RAM.
        // No original opcode, target routine or percentage helper is patched.
        for (offset, byte) in [
            0x18,
            0xFB,
            0xE2,
            0x20,
            0xC2,
            0x10,
            0xA9,
            0x7E,
            0x48,
            0xAB,
            0xA2,
            ACTOR as u8,
            (ACTOR >> 8) as u8,
            0x5C,
            entry as u8,
            (entry >> 8) as u8,
            (entry >> 16) as u8,
        ]
        .into_iter()
        .enumerate()
        {
            bus.write8(0x0200 + offset as u32, byte);
        }
        Self {
            bus,
            reset: false,
            stop,
            names,
        }
    }

    fn compare(&mut self, game: &mut Game, player: u16, flight: FlightViewAnchor) {
        let actor = &game.objs.aliens[player as usize];
        let [x, y] = [actor.worldx, actor.worldy];
        let center = game.vars.strategy.view_center_y;
        for (base, field, value) in [
            (ACTOR, "AL_WORLDX", x),
            (ACTOR, "AL_WORLDY", y),
            (0, "VIEWCY", center),
            (0, "PVIEWPOSX", 3141),
            (0, "PVIEWPOSY", -2718),
            (0, "PVIEWPOSZ", DEPTH_SENTINEL),
        ] {
            self.bus
                .write16(WRAM | (base + self.names[field]), value as u16);
        }
        self.bus.write8(
            WRAM | self.names["SPLAYERFLYMODE"],
            game.vars.player_view_mode as u8,
        );
        game.vars.strategy.player_view_position = [3141, -2718, DEPTH_SENTINEL];
        self.reset = true;
        let mut cpu = CPU::new();
        let mut reached = false;
        for _ in 0..400 {
            cpu.cycle(self);
            if cpu.tcu() == 0 {
                let address = (u32::from(cpu.pbr()) << 16) | u32::from(cpu.pc().wrapping_sub(1));
                if address == self.stop {
                    reached = true;
                    break;
                }
            }
        }
        assert!(
            reached,
            "camera tail did not reach original depth-chase call"
        );
        apply_flight_view_anchor(game, player, flight);
        for (index, field) in ["PVIEWPOSX", "PVIEWPOSY", "PVIEWPOSZ"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                game.vars.strategy.player_view_position[index],
                self.bus.read16(WRAM | self.names[field]) as i16,
                "{flight:?} {:?} x={x} y={y} center={center}: {field}",
                game.vars.player_view_mode,
            );
        }
        assert_eq!(self.bus.read16(WRAM | self.names["VIEWCY"]) as i16, center);
        assert_eq!(game.vars.strategy.view_center_y, center);
        assert_eq!(
            self.bus.read16(WRAM | (ACTOR + self.names["AL_WORLDX"])) as i16,
            x
        );
        assert_eq!(
            self.bus.read16(WRAM | (ACTOR + self.names["AL_WORLDY"])) as i16,
            y
        );
        assert_eq!(game.objs.aliens[player as usize].worldx, x);
        assert_eq!(game.objs.aliens[player as usize].worldy, y);
    }
}

#[test]
fn space_camera_matches_every_coordinate_word_and_all_five_camera_modes() {
    let mut original = Original::new("PLAYERINSPACE_STRAT", "DO_PLAYER_LIMITX", 78);
    let mut game = Game::new();
    let player = game.objs.alloc().unwrap();
    for mode in MODES {
        game.vars.player_view_mode = mode;
        for bits in 0..=u16::MAX {
            let actor = &mut game.objs.aliens[player as usize];
            actor.worldx = bits as i16;
            actor.worldy = bits.rotate_left(5) as i16;
            // Space must ignore the live center even when it differs from
            // the source's fixed Space_ViewCY immediate.
            game.vars.strategy.view_center_y = bits.wrapping_mul(31) as i16;
            original.compare(&mut game, player, FlightViewAnchor::Space);
        }
    }
}

fn compare_surface(strategy: &str) {
    let mut original = Original::new(strategy, "DO_PLAYER_YVEL125", 38);
    let mut game = Game::new();
    let player = game.objs.alloc().unwrap();
    for center in [i16::MIN, -32767, -60, -50, -1, 0, 1, i16::MAX] {
        game.vars.strategy.view_center_y = center;
        for bits in 0..=u16::MAX {
            let actor = &mut game.objs.aliens[player as usize];
            actor.worldx = bits as i16;
            actor.worldy = bits.rotate_left(7) as i16;
            game.vars.player_view_mode = MODES[usize::from(bits) % MODES.len()];
            original.compare(&mut game, player, FlightViewAnchor::Surface);
        }
    }
}

#[test]
fn water_camera_matches_every_coordinate_word_and_live_center_wrap_boundaries() {
    compare_surface("PLAYERONWATER_STRAT");
}

#[test]
fn planet_camera_matches_every_coordinate_word_and_live_center_wrap_boundaries() {
    compare_surface("PLAYERONPLANET_STRAT");
}

#[test]
fn underground_camera_matches_every_horizontal_and_center_word() {
    let mut original = Original::new("PLAYERUNDERGND_STRAT", "DO_PLAYERYVELD2", 27);
    let mut game = Game::new();
    let player = game.objs.alloc().unwrap();
    for mode in MODES {
        game.vars.player_view_mode = mode;
        for bits in 0..=u16::MAX {
            let actor = &mut game.objs.aliens[player as usize];
            actor.worldx = bits as i16;
            actor.worldy = bits.rotate_left(9) as i16;
            game.vars.strategy.view_center_y = bits.rotate_left(3) as i16;
            original.compare(&mut game, player, FlightViewAnchor::Underground);
        }
    }
}
