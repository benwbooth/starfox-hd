//! Wire-shield collection, expiry and pre-strategy colour publication.
use super::{
    compare_pickup_state,
    repair_chain::{compare_sound, scene, seed},
    Source, OBJECT, TARGET, WRAM,
};
use sf_game::{alien::ASF_HITFLASH, Game};
use sf_oracle::{call, call_near, Entry, SnesBus};
use sf_strat::{
    enemies_ground::{item6_istrat, wiremandie_istrat},
    player::{select_ship, strat_player, PSHIPNUM_NORM},
};

fn word(source: &Source, bus: &mut SnesBus, base: u32, name: &str) -> u16 {
    let address = WRAM | (base + source.symbol(name));
    u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)])
}

fn source_shape(source: &Source, bus: &mut SnesBus, native: u16) -> u16 {
    let index = match native {
        0 => return 0,
        2 => 0,
        368..=370 => native - 367,
        351..=354 => native - 347,
        _ => panic!("unexpected ship shape {native}"),
    };
    let address = source.symbol("PLAYER_SHAPES") + u32::from(index) * 2;
    u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)])
}

fn fields(game: &Game) -> [(&'static str, u8); 5] {
    [
        ("PNUMHITS", game.vars.strategy.player_hit_count),
        ("CURR_SHIP", game.vars.strategy.player_ship_selection),
        ("SHIELDUP", game.vars.shieldup),
        ("WIREENDFLASH", game.vars.wireendflash),
        ("PSHIPFLAGS2", game.vars.pshipflags2),
    ]
}

fn seed_shield(source: &Source, bus: &mut SnesBus, game: &Game) {
    for (name, value) in fields(game) {
        source.byte(bus, 0, name, value);
    }
    for (name, value) in [
        "PLAYERSHAPE",
        "PLAYERSHAPEL",
        "PLAYERSHAPER",
        "PLAYERSHAPELR",
    ]
    .into_iter()
    .zip(game.vars.strategy.player_shapes)
    {
        let shape = source_shape(source, bus, value);
        source.word(bus, 0, name, shape as i16);
    }
}

fn compare_shield(source: &Source, bus: &mut SnesBus, game: &Game, context: &str) {
    for (name, value) in fields(game) {
        assert_eq!(
            value,
            bus.read8(WRAM | source.symbol(name)),
            "{context}: {name}"
        );
    }
    for (name, value) in [
        "PLAYERSHAPE",
        "PLAYERSHAPEL",
        "PLAYERSHAPER",
        "PLAYERSHAPELR",
    ]
    .into_iter()
    .zip(game.vars.strategy.player_shapes)
    {
        assert_eq!(
            source_shape(source, bus, value),
            word(source, bus, 0, name),
            "{context}: {name}"
        );
    }
}

#[test]
fn wire_pickup_matches_original_wrapped_range_death_and_selection_side_effects() {
    let source = Source::load();
    let edges = [
        -32768i16, -32760, -121, -120, -119, -61, -60, -59, -1, 0, 1, 59, 60, 61, 119, 120, 121,
        32760, 32767,
    ];
    for axis in 0..3 {
        for value in edges {
            for reference in [-32760i16, 0, 32760] {
                for (drift, flags) in [(0, 0x54), (1, 0x54), (0, 0xD4), (1, 0xD4)] {
                    let (mut game, player, id, sounds) = scene();
                    select_ship(&mut game, PSHIPNUM_NORM);
                    game.vars.strategy.player_ship_selection = 6;
                    game.vars.strategy.player_hit_count = 173;
                    game.vars.shieldup = 7;
                    game.vars.wireendflash = 17;
                    game.vars.pshipflags2 = flags;
                    let mut position = [0; 3];
                    position[axis] = reference;
                    let pl = &mut game.objs.aliens[player as usize];
                    [pl.worldx, pl.worldy, pl.worldz] = position;
                    position[axis] = value;
                    let al = &mut game.objs.aliens[id as usize];
                    [al.worldx, al.worldy, al.worldz] = position;
                    al.roty = 254;
                    al.sbyte1 = drift;
                    let mut bus = seed(&source, &game, id);
                    seed_shield(&source, &mut bus, &game);
                    source.run(&mut bus, "ITEM6_ISTRAT");
                    item6_istrat(&mut game, id);
                    let context = format!("wire axis={axis} coordinate={value} reference={reference} drift={drift} flags={flags}");
                    compare_pickup_state(&source, &mut bus, &game, id, &context);
                    compare_shield(&source, &mut bus, &game, &context);
                    compare_sound(&source, &mut bus, &sounds, &context);
                }
            }
        }
    }
}

#[test]
fn wire_drop_matches_original_allocation_and_deferred_visit() {
    const CHILD: u32 = 0x0700;
    let source = Source::load();
    for can_allocate in [false, true] {
        let (mut game, _, id, sounds) = scene();
        let al = &mut game.objs.aliens[id as usize];
        al.sflags = 0;
        al.sflags2 = 0;
        [al.worldx, al.worldy, al.worldz] = [100, 200, 1000];
        let available = game.objs.alloc().unwrap();
        while game.objs.alloc().is_some() {}
        if can_allocate {
            game.objs.free(available);
        }
        let mut bus = seed(&source, &game, id);
        seed_shield(&source, &mut bus, &game);
        source.word(&mut bus, 0, "ALLST", OBJECT as i16);
        if can_allocate {
            source.word(&mut bus, 0, "ALFREELST", CHILD as i16);
        }
        source.run(&mut bus, "WIREMANDIE_ISTRAT");
        wiremandie_istrat(&mut game, id);
        let context = format!("wire drop allocation={can_allocate}");
        compare_pickup_state(&source, &mut bus, &game, id, &context);
        compare_shield(&source, &mut bus, &game, &context);
        compare_sound(&source, &mut bus, &sounds, &context);
        if !can_allocate {
            assert!(!game
                .objs
                .aliens
                .iter()
                .any(|al| al.active && al.shape == 159));
            continue;
        }
        let drop = game.objs.aliens[available as usize];
        assert_eq!(drop.shape, 159);
        assert_eq!([drop.worldx, drop.worldy, drop.worldz], [100, 200, 1000]);
        let installed = u32::from(word(&source, &mut bus, CHILD, "AL_STRATPTR"))
            | u32::from(bus.read8(WRAM | (CHILD + source.symbol("AL_STRATPTR") + 2))) << 16;
        assert_eq!(installed, source.symbol("ITEM6_ISTRAT"));
        assert_eq!(
            drop.sflags2,
            bus.read8(WRAM | (CHILD + source.symbol("AL_SFLAGS2")))
        );
        source.word(&mut bus, 0, "AL1PT", CHILD as i16);
        source.byte(&mut bus, 0, "ALDEAD", 0);
        game.objs.aldead = 0;
        source.run_at(&mut bus, "ITEM6_ISTRAT", CHILD);
        game.call_strat(drop.stratptr.unwrap(), available);
        let after = game.objs.aliens[available as usize];
        assert_eq!(after.shape, 159);
        // Shape handles are decoded catalog IDs; compare actor fields after
        // separately asserting this remains the configured wire pickup mesh.
        let mut translated = after;
        translated.shape = word(&source, &mut bus, CHILD, "AL_SHAPE");
        source.compare_at(&mut bus, CHILD, &translated, &context);
        compare_shield(&source, &mut bus, &game, &context);
    }
}

#[test]
fn wire_expiry_matches_original_hit_byte_and_every_countdown() {
    let source = Source::load();
    for hits in [0u8, 1, 2, 3, 127, 128, 129, 130, 131, 255] {
        for flash in 0..=u8::MAX {
            let (mut game, player, id, _) = scene();
            game.objs.aliens[player as usize].hp = 40;
            game.vars.strategy.player_hit_count = hits;
            game.vars.strategy.player_ship_selection = 1;
            game.vars.pshipflags2 = 2;
            game.vars.shieldup = 7;
            game.vars.wireendflash = flash;
            select_ship(&mut game, 1);
            let mut bus = seed(&source, &game, id);
            seed_shield(&source, &mut bus, &game);
            source.byte(&mut bus, TARGET, "AL_HP", 40);
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
            assert!(exit.returned, "playermove_srou must return");
            strat_player(&mut game, player);
            compare_shield(
                &source,
                &mut bus,
                &game,
                &format!("wire expiry hits={hits} flash={flash}"),
            );
        }
    }
}

#[test]
fn ship_and_hud_color_match_original_every_frame_and_selection_branch() {
    let source = Source::load();
    for color in 0..=u8::MAX {
        for selection in [0, 1, 2, 255] {
            for shield in [0, 2] {
                for hitflash in [0, ASF_HITFLASH] {
                    let (mut game, player, id, _) = scene();
                    game.vars.strategy.player_ship_selection = selection;
                    game.vars.pshipflags2 = shield;
                    let ship = &mut game.objs.aliens[player as usize];
                    ship.colframe = color;
                    ship.sflags = hitflash;
                    game.objs.aliens[0].colframe = 201;
                    let mut bus = seed(&source, &game, id);
                    source.word(&mut bus, 0, "INTERNALPLAYPT", OBJECT as i16);
                    source.word(&mut bus, 0, "DUMMYOBJ", 0x0C00);
                    source.byte(&mut bus, TARGET, "ALX_COLFRAME", color);
                    source.byte(&mut bus, TARGET, "AL_SFLAGS", hitflash);
                    source.byte(&mut bus, 0, "CURR_SHIP", selection);
                    let exit = call(
                        &mut bus,
                        source.symbol("INIT_STRATS_L"),
                        &Entry {
                            p: 0x20,
                            dbr: 0x7E,
                            ..Default::default()
                        },
                    );
                    assert!(exit.returned, "init_strats_l must return");
                    game.run_strategies();
                    let context = format!("ship colour={color} selection={selection} shield={shield} hitflash={hitflash}");
                    assert_eq!(
                        game.objs.aliens[player as usize].colframe,
                        bus.read8(WRAM | (TARGET + source.symbol("ALX_COLFRAME"))),
                        "{context}: ship color"
                    );
                    assert_eq!(
                        game.vars.strategy.cockpit_hud_color,
                        bus.read8(source.symbol("M_HUDCOLOUR")),
                        "{context}: HUD color"
                    );
                    assert_eq!(
                        game.objs.aliens[0].colframe, 201,
                        "{context}: not internal player"
                    );
                }
            }
        }
    }
}
