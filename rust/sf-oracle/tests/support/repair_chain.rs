//! Original GASTRATS repair chain, exercised through the real routine entries.
use super::{compare_pickup, compare_pickup_state, seed_pickup_player, Source, OBJECT, WRAM};
use sf_game::{game::Hooks, Game};
use sf_oracle::SnesBus;
use sf_strat::{
    enemies_ground::ripman_istrat,
    enemy_a::{item4_istrat, item4_strat, ripair_istrat, ripair_strat, strat_item7_init},
};
use std::{cell::RefCell, rc::Rc};

const CHILD: u32 = 0x0700;
const LEFT: u32 = 0x0900;
const RIGHT: u32 = 0x0A00;

#[derive(Clone, Default)]
struct Sounds(Rc<RefCell<Vec<u8>>>);
impl Hooks for Sounds {
    fn play_se(&mut self, sound: u8) {
        self.0.borrow_mut().push(sound);
    }
}

fn scene() -> (Game, u16, u16, Sounds) {
    let sounds = Sounds::default();
    let mut game = Game::with_hooks(Box::new(sounds.clone()));
    let decoy = game.objs.alloc().unwrap();
    game.objs.aliens[decoy as usize].worldz = 12000;
    let player = game.objs.alloc().unwrap();
    game.vars.player_object = player as i16;
    game.vars.internal_playpt = decoy as i16;
    let id = game.objs.alloc().unwrap();
    let left = game.objs.alloc().unwrap();
    let right = game.objs.alloc().unwrap();
    game.coldet.pcbox.lwing = Some(left);
    game.coldet.pcbox.rwing = Some(right);
    game.vars.minpmove_y = -50;
    game.vars.gameframe = 1;
    let al = &mut game.objs.aliens[id as usize];
    al.shape = 123;
    al.sflags = 0xA4;
    al.sflags2 = 0xB0;
    al.type_ = 0xD3;
    al.colframe = 0x83;
    al.count = 91;
    for wing in [left, right] {
        let al = &mut game.objs.aliens[wing as usize];
        al.hp = 2;
        al.ap = 3;
        al.sflags = 0xA4;
        al.sflags2 = 0xB0;
        al.type_ = 0xFF;
    }
    (game, player, id, sounds)
}

fn seed(source: &Source, game: &Game, id: u16) -> SnesBus {
    let mut bus = SnesBus::new(source.rom.clone());
    source.seed(&mut bus, &game.objs.aliens[id as usize], game);
    source.byte(
        &mut bus,
        OBJECT,
        "ALX_COLFRAME",
        game.objs.aliens[id as usize].colframe,
    );
    seed_pickup_player(source, &mut bus, game);
    source.word(&mut bus, 0, "PLAYER_POSX", game.vars.player_posx);
    source.word(&mut bus, 0, "PLAYER_POSY", game.vars.player_posy);
    source.word(&mut bus, 0, "PLAYER_POSZ", game.vars.player_posz);
    source.word(&mut bus, 0, "PCBOXOBJ_LW", LEFT as i16);
    source.word(&mut bus, 0, "PCBOXOBJ_RW", RIGHT as i16);
    for (wing, base) in [
        (game.coldet.pcbox.lwing.unwrap(), LEFT),
        (game.coldet.pcbox.rwing.unwrap(), RIGHT),
    ] {
        let al = &game.objs.aliens[wing as usize];
        for (name, value) in [
            ("AL_HP", al.hp),
            ("AL_AP", al.ap),
            ("AL_SFLAGS", al.sflags),
            ("AL_SFLAGS2", al.sflags2),
            ("AL_TYPE", al.type_),
            ("AL_COLLFLAGS", al.collflags),
        ] {
            source.byte(&mut bus, base, name, value);
        }
    }
    bus
}

fn compare_sound(source: &Source, bus: &mut SnesBus, sounds: &Sounds, context: &str) {
    let count = bus.read8(WRAM | source.symbol("SDSPT3"));
    let expected: Vec<_> = (0..count)
        .map(|slot| bus.read8(WRAM | (source.symbol("SDPORT3") + u32::from(slot))))
        .collect();
    assert_eq!(*sounds.0.borrow(), expected, "{context}: sound queue");
}

fn pointer(source: &Source, bus: &mut SnesBus, base: u32, name: &str) -> u32 {
    let address = WRAM | (base + source.symbol(name));
    u32::from(bus.read8(address))
        | u32::from(bus.read8(address + 1)) << 8
        | u32::from(bus.read8(address + 2)) << 16
}

#[test]
fn repair_entries_match_original_flag_bytes_initial_motion_and_published_position() {
    let source = Source::load();
    for (name, native) in [
        ("ITEM4_ISTRAT", item4_istrat as fn(&mut Game, u16)),
        ("RIPAIR_ISTRAT", ripair_istrat),
        ("RIPMAN_ISTRAT", ripman_istrat),
    ] {
        for value in [-32768i16, -32760, -31, -30, -29, 0, 32737, 32738, 32767] {
            let (mut game, _, id, sounds) = scene();
            [
                game.vars.player_posx,
                game.vars.player_posy,
                game.vars.player_posz,
            ] = [value, value.wrapping_add(13), value.wrapping_add(29)];
            let al = &mut game.objs.aliens[id as usize];
            [al.worldx, al.worldy, al.worldz] = [value; 3];
            [al.rotx, al.roty, al.rotz] = [251, 252, 253];
            [al.vx, al.vy, al.vz] = [41, 43, 47];
            let mut bus = seed(&source, &game, id);
            source.run(&mut bus, name);
            native(&mut game, id);
            let context = format!("{name} coordinate={value}");
            source.compare(&mut bus, &game.objs.aliens[id as usize], &context);
            compare_sound(&source, &mut bus, &sounds, &context);
        }
    }
}

#[test]
fn repair_and_broken_wing_pickups_match_original_full_pool_wrapping_distance_and_death_paths() {
    let source = Source::load();
    let edges = [
        -32768i16, -32760, -121, -120, -119, -101, -100, -99, -61, -60, -59, -1, 0, 1, 59, 60, 61,
        99, 100, 101, 119, 120, 121, 32760, 32767,
    ];
    for (name, native) in [
        ("ITEM4_STRAT", item4_strat as fn(&mut Game, u16)),
        ("ITEM7_ISTRAT", strat_item7_init),
    ] {
        for axis in 0..3 {
            for value in edges {
                for reference in [-32760i16, 0, 32760] {
                    for (drift, death_flags) in [(0, 0), (1, 0), (0, 128), (1, 128)] {
                        let (mut game, player, id, sounds) = scene();
                        let mut position = [0; 3];
                        position[axis] = reference;
                        let pl = &mut game.objs.aliens[player as usize];
                        [pl.worldx, pl.worldy, pl.worldz] = position;
                        position[axis] = value;
                        let al = &mut game.objs.aliens[id as usize];
                        [al.worldx, al.worldy, al.worldz] = position;
                        [al.rotx, al.roty, al.rotz] = [253, 254, 255];
                        al.sbyte1 = drift;
                        game.vars.pshipflags = 0xFF;
                        game.vars.pshipflags2 = death_flags;
                        game.vars.minpmove_y = reference.wrapping_sub(50);
                        let mut bus = seed(&source, &game, id);
                        while game.objs.alloc().is_some() {}
                        source.run(&mut bus, name);
                        native(&mut game, id);
                        let context = format!("{name} axis={axis} coordinate={value} reference={reference} drift={drift} death={death_flags}");
                        compare_pickup(&source, &mut bus, &game, id, &context);
                        compare_sound(&source, &mut bus, &sounds, &context);
                    }
                }
            }
        }
    }
}

#[test]
fn repair_motion_and_catch_match_original_wrapped_coordinates_countdown_and_wing_restoration() {
    let source = Source::load();
    let edges = [
        -32768i16, -32760, -501, -500, -499, -31, -30, -29, -21, -20, -19, -1, 0, 1, 19, 20, 21,
        29, 30, 31, 499, 500, 501, 32760, 32767,
    ];
    for axis in 0..3 {
        for value in edges {
            for reference in [-32760i16, 0, 32760] {
                for countdown in [0, 1, 2, 30, 255] {
                    let (mut game, player, id, sounds) = scene();
                    let mut position = [0; 3];
                    position[axis] = reference;
                    let pl = &mut game.objs.aliens[player as usize];
                    [pl.worldx, pl.worldy, pl.worldz] = position;
                    pl.rotz = value as u8;
                    [
                        game.vars.player_posx,
                        game.vars.player_posy,
                        game.vars.player_posz,
                    ] = position;
                    // Published coordinates intentionally differ from actor
                    // coordinates: approaches use one, collision uses the other.
                    game.vars.player_posx = game.vars.player_posx.wrapping_add(7);
                    game.vars.player_posy = game.vars.player_posy.wrapping_sub(3);
                    game.vars.pviewvelz = reference;
                    game.vars.pshipflags = 0xFF;
                    position[axis] = value;
                    let al = &mut game.objs.aliens[id as usize];
                    [al.worldx, al.worldy, al.worldz] = position;
                    al.vz = 30;
                    al.rotz = reference as u8;
                    al.sbyte1 = countdown;
                    let original_count = al.count;
                    let original_color = al.colframe;
                    let mut bus = seed(&source, &game, id);
                    source.run(&mut bus, "RIPAIR_STRAT");
                    ripair_strat(&mut game, id);
                    let context = format!("repair axis={axis} coordinate={value} reference={reference} countdown={countdown}");
                    source.compare(&mut bus, &game.objs.aliens[id as usize], &context);
                    compare_sound(&source, &mut bus, &sounds, &context);
                    assert_eq!(
                        game.vars.pshipflags,
                        bus.read8(WRAM | source.symbol("PSHIPFLAGS")),
                        "{context}: ship flags"
                    );
                    for (wing, base) in [
                        (game.coldet.pcbox.lwing.unwrap(), LEFT),
                        (game.coldet.pcbox.rwing.unwrap(), RIGHT),
                    ] {
                        source.compare_at(
                            &mut bus,
                            base,
                            &game.objs.aliens[wing as usize],
                            &context,
                        );
                    }
                    assert_eq!(
                        game.objs.aliens[id as usize].count, original_count,
                        "flash entry is deferred"
                    );
                    assert_eq!(game.objs.aliens[id as usize].colframe, original_color);
                    let installed = pointer(&source, &mut bus, OBJECT, "AL_STRATPTR");
                    assert_eq!(
                        game.objs.aliens[id as usize].stratptr.is_some(),
                        installed == source.symbol("FLASHPLAYER_ISTRAT"),
                        "{context}: flash installation"
                    );
                }
            }
        }
    }
}

#[test]
fn repair_pickup_installs_child_without_entering_it_or_playing_approach_sound() {
    let source = Source::load();
    for (name, native) in [
        ("ITEM4_STRAT", item4_strat as fn(&mut Game, u16)),
        ("ITEM7_ISTRAT", strat_item7_init),
    ] {
        for death in [0, 128] {
            let (mut game, _, id, sounds) = scene();
            game.vars.pshipflags = 0xFF;
            game.vars.pshipflags2 = death;
            let mut bus = seed(&source, &game, id);
            source.word(&mut bus, 0, "ALLST", OBJECT as i16);
            source.word(&mut bus, 0, "ALFREELST", CHILD as i16);
            source.run(&mut bus, name);
            native(&mut game, id);
            compare_pickup(&source, &mut bus, &game, id, name);
            compare_sound(&source, &mut bus, &sounds, name);
            assert_eq!(
                pointer(&source, &mut bus, CHILD, "AL_STRATPTR"),
                source.symbol("RIPAIR_ISTRAT")
            );
            let active = game.objs.active_indices();
            let after = active[active.iter().position(|&actor| actor == id).unwrap() + 1];
            let child = &game.objs.aliens[after as usize];
            assert_eq!(child.shape, 401);
            assert_eq!([child.worldx, child.worldy, child.worldz], [0; 3]);
            assert_eq!(child.sbyte1, 0);
            assert_eq!(bus.read8(WRAM | (CHILD + source.symbol("AL_SBYTE1"))), 0);
            let init = child.stratptr.unwrap();
            game.call_strat(init, after);
            assert_eq!(game.objs.aliens[after as usize].sbyte1, 30);
            assert_eq!(*sounds.0.borrow(), [0x8B]);
        }
    }
}

#[test]
fn intact_laser_pickup_matches_original_upgrade_score_and_flag_preserving_wing_entries() {
    use sf_game::vars::PSF2_PLAYERHP0;
    use sf_strat::enemy_a::{PSF2_DOUBLASER, PSF3_BEAMBALL, PSF_BRKLWING, PSF_BRKRWING};
    let source = Source::load();
    for flags in 0..=255 {
        if flags & (PSF_BRKLWING | PSF_BRKRWING) != 0 {
            continue;
        }
        for upgrade in [
            0,
            PSF2_DOUBLASER,
            PSF2_PLAYERHP0,
            PSF2_DOUBLASER | PSF2_PLAYERHP0,
        ] {
            let (mut game, _, id, sounds) = scene();
            game.vars.pshipflags = flags;
            game.vars.pshipflags2 = upgrade;
            game.vars.pshipflags3 = 0xFF & !PSF3_BEAMBALL;
            game.vars.shared.player_score = u16::MAX - u16::from(flags);
            let mut bus = seed(&source, &game, id);
            source.byte(&mut bus, 0, "PSHIPFLAGS3", game.vars.pshipflags3);
            source.word(
                &mut bus,
                0,
                "PLAYERSCORE",
                game.vars.shared.player_score as i16,
            );
            source.run(&mut bus, "ITEM7_ISTRAT");
            strat_item7_init(&mut game, id);
            let context = format!("intact pickup flags={flags} upgrade={upgrade}");
            compare_pickup_state(&source, &mut bus, &game, id, &context);
            compare_sound(&source, &mut bus, &sounds, &context);
            for (wing, base) in [
                (game.coldet.pcbox.lwing.unwrap(), LEFT),
                (game.coldet.pcbox.rwing.unwrap(), RIGHT),
            ] {
                source.compare_at(&mut bus, base, &game.objs.aliens[wing as usize], &context);
            }
            assert_eq!(
                game.vars.pshipflags2,
                bus.read8(WRAM | source.symbol("PSHIPFLAGS2")),
                "{context}"
            );
            assert_eq!(
                game.vars.pshipflags3,
                bus.read8(WRAM | source.symbol("PSHIPFLAGS3")),
                "{context}"
            );
            let score = WRAM | source.symbol("PLAYERSCORE");
            assert_eq!(
                game.vars.shared.player_score,
                u16::from_le_bytes([bus.read8(score), bus.read8(score + 1)]),
                "{context}"
            );
        }
    }
}
