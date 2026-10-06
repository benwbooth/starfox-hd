//! Execute source-built SF1 weapon routines, including their initializer tails.
//! Expected state comes from the original instructions, never a native trace.

use std::collections::HashMap;

use sf_game::{alien::Alien, Game};
use sf_oracle::{
    call, inject_runmario_trampoline, load_built_rom, load_symbols, Entry, SnesBus,
    BUILT_RUNMARIO_L_ROM,
};
use sf_strat::enemy_a::{
    flashplayer_istrat, flatmiss_istrat, helpball_istrat, helpball_strat, helpballhome_istrat,
    item7a_istrat, relflatmiss_istrat,
};

// Keep clear of the call harness's bootstrap and return trap.
const OBJECT: u32 = 0x0100;
const TARGET: u32 = 0x0500;
const MOTHER: u32 = 0x0600;
const WRAM: u32 = 0x7E_0000;

#[path = "support/repair_chain.rs"]
mod repair_chain;

#[path = "support/special_pickup.rs"]
mod special_pickup;

#[path = "support/body_pickup.rs"]
mod body_pickup;

struct Source {
    rom: Vec<u8>,
    symbols: HashMap<String, u32>,
}

fn seed_pickup_player(source: &Source, bus: &mut SnesBus, game: &Game) {
    let player = &game.objs.aliens[game.player_object().unwrap() as usize];
    source.word(bus, 0, "PLAYPT", TARGET as i16);
    for (name, value) in [
        ("AL_WORLDX", player.worldx),
        ("AL_WORLDY", player.worldy),
        ("AL_WORLDZ", player.worldz),
    ] {
        source.word(bus, TARGET, name, value);
    }
    for (name, value) in [
        ("AL_ROTX", player.rotx),
        ("AL_ROTY", player.roty),
        ("AL_ROTZ", player.rotz),
    ] {
        source.byte(bus, TARGET, name, value);
    }
    source.byte(bus, 0, "PSHIPFLAGS", game.vars.pshipflags);
    source.byte(bus, 0, "PSHIPFLAGS2", game.vars.pshipflags2);
    source.byte(bus, 0, "PSHIPFLAGS3", game.vars.pshipflags3);
    source.word(bus, 0, "MINPMOVEY", game.vars.minpmove_y);
    source.word(bus, 0, "GAMEFRAME", game.vars.gameframe as i16);
    source.word(bus, 0, "ALFREELST", 0);
}

fn compare_pickup_state(source: &Source, bus: &mut SnesBus, game: &Game, id: u16, context: &str) {
    let mut translated = game.objs.aliens[id as usize];
    // Catalog IDs are decoded native shape handles, not source addresses.
    if let Some(address) = match translated.shape {
        351..=354 => Some(source.symbol("PWIRESHAPES") + u32::from(translated.shape - 351) * 2),
        0 => Some(source.symbol("PNULLSHAPES")),
        _ => None,
    } {
        // Shape labels appear twice in the linker map (header and graphics
        // data). Read the original gameplay table, not the ambiguous label.
        translated.shape = u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
    }
    source.compare(bus, &translated, context);
    assert_eq!(
        translated.colframe,
        bus.read8(WRAM | (OBJECT + source.symbol("ALX_COLFRAME"))),
        "{context}: color"
    );
    assert_eq!(
        game.objs.aldead,
        bus.read8(WRAM | source.symbol("ALDEAD")),
        "{context}: removal"
    );
    assert_eq!(
        game.vars.pshipflags,
        bus.read8(WRAM | source.symbol("PSHIPFLAGS")),
        "{context}: ship flags"
    );
    for (field, actual) in [
        ("PSHIPFLAGS2", game.vars.pshipflags2),
        ("PSHIPFLAGS3", game.vars.pshipflags3),
    ] {
        assert_eq!(
            actual,
            bus.read8(WRAM | source.symbol(field)),
            "{context}: {field}"
        );
    }
}

fn compare_pickup(source: &Source, bus: &mut SnesBus, game: &Game, id: u16, context: &str) {
    compare_pickup_state(source, bus, game, id, context);
    assert_eq!(
        bus.read8(WRAM | source.symbol("SDSPT3")),
        0,
        "full pool must not play pickup sound"
    );
}

#[test]
fn flash_entry_and_following_visits_match_source_for_every_color_and_lifetime_byte() {
    let source = Source::load();
    for initial in [false, true] {
        for color in 0..=255 {
            for frame in [0, 1] {
                let mut game = Game::new();
                game.objs.alloc().unwrap(); // decoy slot zero
                let player = game.objs.alloc().unwrap();
                game.vars.player_object = player as i16;
                let pl = &mut game.objs.aliens[player as usize];
                [pl.worldx, pl.worldy, pl.worldz] = [32760, -32760, 812];
                [pl.rotx, pl.roty, pl.rotz] = [7, 19, 31];
                let id = game.objs.alloc().unwrap();
                flashplayer_istrat(&mut game, id);
                // Keep the installed continuation for direct non-entry calls.
                let strategy = game.objs.aliens[id as usize].stratptr.unwrap();
                game.objs.aldead = 0;
                let al = &mut game.objs.aliens[id as usize];
                al.shape = 123;
                al.count = color;
                al.colframe = color;
                al.sflags = 0xA4;
                al.sflags2 = 0xB0;
                [al.worldx, al.worldy, al.worldz] = [10, 20, 30];
                game.vars.pshipflags = color;
                game.vars.gameframe = frame;
                let mut bus = SnesBus::new(source.rom.clone());
                source.seed(&mut bus, &game.objs.aliens[id as usize], &game);
                source.byte(&mut bus, OBJECT, "ALX_COLFRAME", color);
                seed_pickup_player(&source, &mut bus, &game);
                let name = if initial {
                    "FLASHPLAYER_ISTRAT"
                } else {
                    "FLASHPLAYER_STRAT"
                };
                source.run(&mut bus, name);
                if initial {
                    flashplayer_istrat(&mut game, id);
                } else {
                    game.call_strat(strategy, id);
                }
                compare_pickup(
                    &source,
                    &mut bus,
                    &game,
                    id,
                    &format!("{name} color={color} frame={frame}"),
                );
            }
        }
    }
}

#[test]
fn flash_entry_matches_source_cockpit_and_player_death_read_order() {
    use sf_core::player_view::PlayerViewMode;
    use sf_game::vars::PSF2_PLAYERHP0;
    let source = Source::load();
    for cockpit in [false, true] {
        for dead in [false, true] {
            for frame in [0, 1] {
                let mut game = Game::new();
                let player = game.objs.alloc().unwrap();
                let id = game.objs.alloc().unwrap();
                let ship = &mut game.objs.aliens[player as usize];
                [ship.worldx, ship.worldy, ship.worldz] = [14, -27, 185];
                [ship.rotx, ship.roty, ship.rotz] = [32, 54, 76];
                let item = &mut game.objs.aliens[id as usize];
                item.shape = 123;
                item.count = 201;
                item.colframe = 0x85;
                item.sflags = 0xA4;
                item.sflags2 = 0xB0;
                if cockpit {
                    game.vars.player_view_mode = PlayerViewMode::Cockpit;
                }
                if dead {
                    game.vars.pshipflags2 = PSF2_PLAYERHP0;
                }
                game.vars.gameframe = frame;
                let mut bus = SnesBus::new(source.rom.clone());
                source.seed(&mut bus, &game.objs.aliens[id as usize], &game);
                source.byte(&mut bus, OBJECT, "ALX_COLFRAME", 0x85);
                seed_pickup_player(&source, &mut bus, &game);
                if cockpit {
                    source.byte(
                        &mut bus,
                        0,
                        "SPLAYERFLYMODE",
                        source.symbol("SPFM_INSIDE") as u8,
                    );
                }
                source.run(&mut bus, "FLASHPLAYER_ISTRAT");
                flashplayer_istrat(&mut game, id);
                compare_pickup(
                    &source,
                    &mut bus,
                    &game,
                    id,
                    &format!("cockpit={cockpit} dead={dead} frame={frame}"),
                );
            }
        }
    }
}

#[test]
fn pickup_entry_matches_source_wrapped_height_range_and_full_pool_paths() {
    let source = Source::load();
    let edges = [
        -32768i16, -32760, -121, -120, -119, -61, -60, -59, -1, 0, 1, 59, 60, 61, 119, 120, 121,
        32760, 32767,
    ];
    for axis in 0..3 {
        for value in edges {
            for reference in [-32760, 0, 32760] {
                for (drift, death_flags) in [(0, 0), (1, 0), (0, 128), (1, 128)] {
                    let mut game = Game::new();
                    let player = game.objs.alloc().unwrap();
                    let id = game.objs.alloc().unwrap();
                    let mut position = [0i16; 3];
                    position[axis] = reference;
                    let pl = &mut game.objs.aliens[player as usize];
                    [pl.worldx, pl.worldy, pl.worldz] = position;
                    position[axis] = value;
                    let al = &mut game.objs.aliens[id as usize];
                    [al.worldx, al.worldy, al.worldz] = position;
                    al.sbyte1 = drift;
                    al.sflags = 0xA4;
                    al.sflags2 = 0xB0;
                    al.shape = 123;
                    al.colframe = 0x83;
                    [al.roty, al.rotz] = [254, 255];
                    game.vars.minpmove_y = reference.wrapping_sub(50);
                    game.vars.pshipflags = 0xFF;
                    game.vars.pshipflags2 = death_flags;
                    game.vars.gameframe = 1;
                    let mut bus = SnesBus::new(source.rom.clone());
                    source.seed(&mut bus, &game.objs.aliens[id as usize], &game);
                    source.byte(&mut bus, OBJECT, "ALX_COLFRAME", 0x83);
                    seed_pickup_player(&source, &mut bus, &game);
                    while game.objs.alloc().is_some() {}
                    source.run(&mut bus, "ITEM7A_ISTRAT");
                    item7a_istrat(&mut game, id);
                    compare_pickup(
                        &source,
                        &mut bus,
                        &game,
                        id,
                        &format!(
                            "axis={axis} coordinate={value} reference={reference} drift={drift} death={death_flags}"
                        ),
                    );
                }
            }
        }
    }
}

#[test]
fn successful_pickup_matches_source_wing_gameplay_data_and_preserves_all_ship_flags() {
    const CHILD: u32 = 0x0700;
    const LEFT: u32 = 0x0900;
    const RIGHT: u32 = 0x0A00;
    let source = Source::load();
    for flags in 0..=255 {
        let mut game = Game::new();
        game.objs.alloc().unwrap(); // live exposed player at origin
        let id = game.objs.alloc().unwrap();
        let left = game.objs.alloc().unwrap();
        let right = game.objs.alloc().unwrap();
        game.coldet.pcbox.lwing = Some(left);
        game.coldet.pcbox.rwing = Some(right);
        game.vars.minpmove_y = -50;
        game.vars.pshipflags = flags;
        game.objs.aliens[id as usize].sbyte1 = 1;
        let mut bus = SnesBus::new(source.rom.clone());
        source.seed(&mut bus, &game.objs.aliens[id as usize], &game);
        seed_pickup_player(&source, &mut bus, &game);
        source.word(&mut bus, 0, "ALLST", OBJECT as i16);
        source.word(&mut bus, 0, "ALFREELST", CHILD as i16);
        source.word(&mut bus, 0, "PCBOXOBJ_LW", LEFT as i16);
        source.word(&mut bus, 0, "PCBOXOBJ_RW", RIGHT as i16);
        source.byte(&mut bus, 0, "TRANS_FLAG", flags);
        for (wing, base) in [(left, LEFT), (right, RIGHT)] {
            let al = &mut game.objs.aliens[wing as usize];
            al.hp = flags;
            al.ap = 0xEA;
            al.type_ = flags;
            al.sflags = flags;
            al.sflags2 = flags;
            for (name, value) in [
                ("AL_HP", flags),
                ("AL_AP", 0xEA),
                ("AL_TYPE", flags),
                ("AL_SFLAGS", flags),
                ("AL_SFLAGS2", flags),
                ("AL_COLLFLAGS", al.collflags),
            ] {
                source.byte(&mut bus, base, name, value);
            }
        }
        source.run(&mut bus, "ITEM7A_ISTRAT");
        item7a_istrat(&mut game, id);
        for (wing, base) in [(left, LEFT), (right, RIGHT)] {
            source.compare_at(
                &mut bus,
                base,
                &game.objs.aliens[wing as usize],
                &format!("wing={wing} flags={flags}"),
            );
        }
        assert_eq!(
            game.vars.pshipflags,
            bus.read8(WRAM | source.symbol("PSHIPFLAGS"))
        );
        assert_eq!(game.vars.pshipflags, flags);
        assert_eq!(
            bus.read8(WRAM | source.symbol("SDSPT3")),
            1,
            "flags={flags}"
        );
        assert_eq!(bus.read8(WRAM | source.symbol("SDPORT3")), 0x10);
        assert_eq!(
            game.objs.aliens[id as usize].count,
            bus.read8(WRAM | (OBJECT + source.symbol("AL_COUNT")))
        );
        // PSTRATS writes the display-transfer phase, not literal zero, into
        // unused left-wing scratch. Static source tests prove no reachable
        // wing routine consumes it, so no hardware snapshot is ported.
        assert_eq!(
            bus.read8(WRAM | (LEFT + source.symbol("ALX_STRATSTATE"))),
            flags
        );
        let pointer = WRAM | (CHILD + source.symbol("AL_STRATPTR"));
        let installed = u32::from(bus.read8(pointer))
            | (u32::from(bus.read8(pointer + 1)) << 8)
            | (u32::from(bus.read8(pointer + 2)) << 16);
        assert_eq!(installed, source.symbol("HELPBALL_ISTRAT"));
    }
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
        // Xanglexy_l/Yanglexy_l use the original Super FX arctangent service.
        bus.enable_gsu();
        // The original boot copies this exact routine from ROM into WRAM.
        inject_runmario_trampoline(bus, BUILT_RUNMARIO_L_ROM, self.symbol("RUNMARIO_L"));
        self.word(bus, 0, "AL1PT", OBJECT as i16);
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
            ("ALX_SND2", object.snd2),
            ("ALX_TX", object.tx),
        ] {
            self.byte(bus, OBJECT, name, value);
        }
        for (name, value) in [
            ("AL_WORLDX", object.worldx),
            ("AL_SHAPE", object.shape as i16),
            ("AL_WORLDY", object.worldy),
            ("AL_WORLDZ", object.worldz),
            ("AL_VX", object.vx),
            ("AL_VY", object.vy),
            ("AL_VZ", object.vz),
            ("ALX_DEPTHOFFSET", object.depthoffset),
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
        self.run_at(bus, name, OBJECT);
    }

    fn run_at(&self, bus: &mut SnesBus, name: &str, object: u32) {
        let exit = call(
            bus,
            self.symbol(name),
            &Entry {
                x: object as u16,
                p: 0x20,
                dbr: 0x7E, // TRANS.dostrats owns extended actor data in WRAM.
                ..Default::default()
            },
        );
        assert!(exit.returned, "{name} did not reach its original return");
        assert_eq!(
            exit.x, object as u16,
            "{name} must preserve the current object"
        );
    }

    fn compare(&self, bus: &mut SnesBus, actual: &Alien, context: &str) {
        self.compare_at(bus, OBJECT, actual, context);
    }

    fn compare_at(&self, bus: &mut SnesBus, base: u32, actual: &Alien, context: &str) {
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
            ("ALX_SND2", actual.snd2),
            ("ALX_TX", actual.tx),
        ] {
            let expected = bus.read8(WRAM | (base + self.symbol(name)));
            assert_eq!(value, expected, "{context}: {name}");
        }
        for (name, value) in [
            ("AL_WORLDX", actual.worldx),
            ("AL_SHAPE", actual.shape as i16),
            ("AL_WORLDY", actual.worldy),
            ("AL_WORLDZ", actual.worldz),
            ("AL_VX", actual.vx),
            ("AL_VY", actual.vy),
            ("AL_VZ", actual.vz),
            ("ALX_DEPTHOFFSET", actual.depthoffset),
        ] {
            let address = WRAM | (base + self.symbol(name));
            let expected = i16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
            assert_eq!(value, expected, "{context}: {name}");
        }
    }
}

#[test]
fn homing_helpball_initializer_preserves_visual_aim_separation_and_enters_motion() {
    let source = Source::load();
    for target_position in [[0, 0, 600], [300, -70, 2000], [-650, 340, -999]] {
        for scroll in [65, -33] {
            let mut game = Game::new();
            let target = game.objs.alloc().unwrap();
            let id = game.objs.alloc().unwrap();
            game.vars.pviewvelz = scroll;
            game.vars.strategy.view_pitch = 1023;
            game.vars.strategy.view_yaw = -1537;
            game.vars.strategy.player_turn_rotation = 2305;
            let [x, y, z] = target_position;
            let other = &mut game.objs.aliens[target as usize];
            [other.worldx, other.worldy, other.worldz] = [x, y, z];
            let object = &mut game.objs.aliens[id as usize];
            [object.worldx, object.worldy, object.worldz] = [10, -30, 70];
            [object.rotx, object.roty, object.rotz] = [37, 45, 67];
            [object.sbyte1, object.sbyte2, object.sbyte3] = [100, 120, 99];
            object.ptr = target + 1;
            object.count = 255;
            object.sflags = 0x8B;
            object.sflags4 = 0xFF;
            object.type_ = 0xA8;
            object.collflags = 0x55;
            object.tx = 77;
            object.depthoffset = 0x4567;
            let object = *object;
            let mut bus = SnesBus::new(source.rom.clone());
            source.seed(&mut bus, &object, &game);
            source.word(&mut bus, OBJECT, "AL_PTR", TARGET as i16);
            for (name, value) in [("AL_WORLDX", x), ("AL_WORLDY", y), ("AL_WORLDZ", z)] {
                source.word(&mut bus, TARGET, name, value);
            }
            source.run(&mut bus, "HELPBALLHOME_ISTRAT");
            let runs = bus.gsu_recent_runs();
            assert_eq!(
                runs.len(),
                2,
                "pitch and yaw execute original GSU arctangent calls"
            );
            assert!(runs.iter().all(|run| !run.hit_limit));
            helpballhome_istrat(&mut game, id);
            source.compare(
                &mut bus,
                &game.objs.aliens[id as usize],
                &format!("helpball target={target_position:?} scroll={scroll}"),
            );
        }
    }
}

#[test]
fn helpball_missing_target_still_initializes_and_releases_its_mother_on_entry() {
    let source = Source::load();
    for active_homes in [0, 1, 2, 255] {
        let mut game = Game::new();
        let mother = game.objs.alloc().unwrap();
        let id = game.objs.alloc().unwrap();
        game.objs.aliens[mother as usize].sbyte1 = active_homes;
        game.objs.aliens[id as usize].sword1 = mother as i16;
        let mut bus = SnesBus::new(source.rom.clone());
        source.seed(&mut bus, &game.objs.aliens[id as usize], &game);
        source.word(&mut bus, OBJECT, "AL_PTR", 0);
        source.word(&mut bus, OBJECT, "AL_SWORD1", MOTHER as i16);
        source.byte(&mut bus, MOTHER, "AL_SBYTE1", active_homes);
        source.run(&mut bus, "HELPBALLHOME_ISTRAT");
        helpballhome_istrat(&mut game, id);
        source.compare(
            &mut bus,
            &game.objs.aliens[id as usize],
            "missing helpball target",
        );
        assert_eq!(game.objs.aldead, bus.read8(WRAM | source.symbol("ALDEAD")));
        assert_eq!(
            game.objs.aliens[mother as usize].sbyte1,
            bus.read8(WRAM | (MOTHER + source.symbol("AL_SBYTE1")))
        );
    }
}

#[test]
fn orbiting_helpball_entry_and_terminal_visit_match_source_without_resetting_counters() {
    let source = Source::load();
    for (name, run) in [
        ("HELPBALL_ISTRAT", helpball_istrat as fn(&mut Game, u16)),
        ("HELPBALL_STRAT", helpball_strat),
    ] {
        for shots in [0, 9, 10, 137, 138, 255] {
            for radius in [30, 117, 120, 255] {
                let mut game = Game::new();
                let player = game.objs.alloc().unwrap();
                let id = game.objs.alloc().unwrap();
                game.vars.strategy.view_pitch = 1023;
                game.vars.strategy.view_yaw = -1537;
                game.vars.strategy.player_turn_rotation = 2305;
                let pl = &mut game.objs.aliens[player as usize];
                [pl.worldx, pl.worldy, pl.worldz] = [32760, -32760, 1000];
                let object = &mut game.objs.aliens[id as usize];
                object.sbyte1 = 2;
                object.sbyte2 = shots;
                object.sbyte3 = radius;
                object.rotz = 67;
                object.sflags = 0x80;
                object.sflags2 = 0x80;
                object.sflags4 = 0xFF;
                object.depthoffset = 0x4567;
                object.tx = 99;
                object.shape = 226;
                let object = *object;
                let mut bus = SnesBus::new(source.rom.clone());
                source.seed(&mut bus, &object, &game);
                source.word(&mut bus, 0, "PLAYPT", TARGET as i16);
                source.word(&mut bus, TARGET, "AL_WORLDX", 32760);
                source.word(&mut bus, TARGET, "AL_WORLDY", -32760);
                source.word(&mut bus, TARGET, "AL_WORLDZ", 1000);
                // No searchable targets; test the entry, orbit and retirement
                // arithmetic independently of allocation and target traversal.
                source.word(&mut bus, 0, "ALLST", 0);
                source.run(&mut bus, name);
                run(&mut game, id);
                source.compare(
                    &mut bus,
                    &game.objs.aliens[id as usize],
                    &format!("{name} shots={shots} radius={radius}"),
                );
                assert_eq!(game.objs.aldead, bus.read8(WRAM | source.symbol("ALDEAD")));
            }
        }
    }
}

#[test]
fn helpball_source_target_flags_and_failed_allocation_keep_the_same_lock_state() {
    use sf_game::alien::{ASF3_LOCKON, ASF3_REALOBJ};
    let source = Source::load();
    for (flags, flags2, flags3, collision) in [
        (0, 0, 0, 0),
        (0x10, 0, 0, 0),
        (0x40, 0, 0, 0),
        (0, 1, 0, 0),
        (0, 0, 0x20, 0),
        (0, 0, 0x10, 0),
        (0, 0, 0, 0x80),
    ] {
        let mut game = Game::new();
        let player = game.objs.alloc().unwrap();
        let target = game.objs.alloc().unwrap();
        let id = game.objs.alloc().unwrap();
        game.objs.aliens[player as usize].worldz = 1000;
        game.objs.aliens[id as usize].sbyte3 = 30;
        let other = &mut game.objs.aliens[target as usize];
        [other.worldx, other.worldy, other.worldz] = [500, 0, 1000];
        other.flags = 16;
        other.hp = 5;
        other.sflags = flags;
        other.sflags2 = flags2;
        other.sflags3 = flags3 | ASF3_REALOBJ;
        other.collflags = collision;
        while game.objs.alloc().is_some() {}

        let mut bus = SnesBus::new(source.rom.clone());
        source.seed(&mut bus, &game.objs.aliens[id as usize], &game);
        source.word(&mut bus, 0, "PLAYPT", MOTHER as i16);
        source.word(&mut bus, MOTHER, "AL_WORLDZ", 1000);
        source.word(&mut bus, 0, "ALLST", TARGET as i16);
        source.word(&mut bus, 0, "ALFREELST", 0);
        source.word(&mut bus, TARGET, "_NEXT", 0);
        source.word(&mut bus, TARGET, "AL_WORLDX", 500);
        source.word(&mut bus, TARGET, "AL_WORLDZ", 1000);
        for (name, value) in [
            ("AL_FLAGS", 16),
            ("AL_HP", 5),
            ("AL_SFLAGS", flags),
            ("AL_SFLAGS2", flags2),
            ("AL_SFLAGS3", flags3 | ASF3_REALOBJ),
            ("AL_COLLFLAGS", collision),
        ] {
            source.byte(&mut bus, TARGET, name, value);
        }
        source.run(&mut bus, "HELPBALL_STRAT");
        helpball_strat(&mut game, id);
        source.compare(
            &mut bus,
            &game.objs.aliens[id as usize],
            "helpball full pool",
        );
        let lock = bus.read8(WRAM | (TARGET + source.symbol("AL_SFLAGS3")));
        assert_eq!(
            game.objs.aliens[target as usize].sflags3, lock,
            "target flags {flags}/{flags2}/{flags3} collision={collision}"
        );
        if (flags2, flags3, collision) == (0, 0, 0) {
            assert_ne!(
                lock & ASF3_LOCKON,
                0,
                "allocation failure retains the source lock"
            );
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
