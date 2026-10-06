//! Original special-weapon pickup inventory, collection and enemy-drop paths.
use super::{
    compare_pickup_state,
    repair_chain::{compare_sound, scene, seed},
    Source, OBJECT, WRAM,
};
use sf_core::player_view::PlayerViewMode;
use sf_oracle::SnesBus;
use sf_strat::enemy_a::{bomwingdie_istrat, strat_item5_init};

fn word(source: &Source, bus: &mut SnesBus, base: u32, name: &str) -> u16 {
    let address = WRAM | (base + source.symbol(name));
    u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)])
}

fn seed_inventory(source: &Source, bus: &mut SnesBus, game: &sf_game::Game) {
    source.word(
        bus,
        0,
        "SPECWEPCNT",
        game.vars.strategy.special_weapon_count as i16,
    );
    source.byte(bus, 0, "SPECFLASH", game.vars.shared.special_flash);
    source.word(bus, 0, "PLAYERSCORE", game.vars.shared.player_score as i16);
}

fn compare_inventory(source: &Source, bus: &mut SnesBus, game: &sf_game::Game, context: &str) {
    assert_eq!(
        game.vars.strategy.special_weapon_count,
        word(source, bus, 0, "SPECWEPCNT"),
        "{context}: inventory"
    );
    assert_eq!(
        game.vars.shared.special_flash,
        bus.read8(WRAM | source.symbol("SPECFLASH")),
        "{context}: flash"
    );
    assert_eq!(
        game.vars.shared.player_score,
        word(source, bus, 0, "PLAYERSCORE"),
        "{context}: score"
    );
}

#[test]
fn special_pickup_matches_original_for_every_inventory_word() {
    let source = Source::load();
    let (mut game, _, id, sounds) = scene();
    let incoming = game.objs.aliens[id as usize];
    let mut bus = seed(&source, &game, id);
    for count in 0..=u16::MAX {
        // Reset every input changed by collection while retaining the ROM
        // allocation; cloning the full bus per word would dominate this test.
        game.objs.aliens[id as usize] = incoming;
        game.objs.aldead = 0;
        sounds.clear();
        game.vars.strategy.special_weapon_count = count;
        game.vars.shared.special_flash = 201;
        game.vars.shared.player_score = 0xBEEF;
        source.seed(&mut bus, &incoming, &game);
        source.byte(&mut bus, OBJECT, "ALX_COLFRAME", incoming.colframe);
        source.byte(&mut bus, 0, "ALDEAD", 0);
        source.byte(&mut bus, 0, "SDSPT3", 0);
        seed_inventory(&source, &mut bus, &game);
        source.run(&mut bus, "ITEM5_ISTRAT");
        strat_item5_init(&mut game, id);
        let context = format!("special pickup count={count}");
        compare_pickup_state(&source, &mut bus, &game, id, &context);
        compare_inventory(&source, &mut bus, &game, &context);
        compare_sound(&source, &mut bus, &sounds, &context);
    }
}

#[test]
fn special_pickup_matches_original_wrapped_range_drift_death_and_cockpit_order() {
    let source = Source::load();
    let edges = [
        -32768i16, -32760, -121, -120, -119, -61, -60, -59, -1, 0, 1, 59, 60, 61, 119, 120, 121,
        32760, 32767,
    ];
    for axis in 0..3 {
        for value in edges {
            for reference in [-32760i16, 0, 32760] {
                for (drift, death) in [(0, 0), (1, 0), (0, 128), (1, 128)] {
                    for cockpit in [false, true] {
                        let (mut game, player, id, sounds) = scene();
                        let mut position = [0; 3];
                        position[axis] = reference;
                        let pl = &mut game.objs.aliens[player as usize];
                        [pl.worldx, pl.worldy, pl.worldz] = position;
                        position[axis] = value;
                        let al = &mut game.objs.aliens[id as usize];
                        [al.worldx, al.worldy, al.worldz] = position;
                        al.sbyte1 = drift;
                        game.vars.pshipflags2 = death;
                        game.vars.strategy.special_weapon_count = 4;
                        game.vars.shared.special_flash = 201;
                        game.vars.shared.player_score = 73;
                        if cockpit {
                            game.vars.player_view_mode = PlayerViewMode::Cockpit;
                        }
                        let mut bus = seed(&source, &game, id);
                        seed_inventory(&source, &mut bus, &game);
                        if cockpit {
                            source.byte(
                                &mut bus,
                                0,
                                "SPLAYERFLYMODE",
                                source.symbol("SPFM_INSIDE") as u8,
                            );
                        }
                        source.run(&mut bus, "ITEM5_ISTRAT");
                        strat_item5_init(&mut game, id);
                        let context = format!("special pickup axis={axis} coordinate={value} reference={reference} drift={drift} death={death} cockpit={cockpit}");
                        compare_pickup_state(&source, &mut bus, &game, id, &context);
                        compare_inventory(&source, &mut bus, &game, &context);
                        compare_sound(&source, &mut bus, &sounds, &context);
                    }
                }
            }
        }
    }
}

#[test]
fn special_drop_matches_original_allocation_failure_and_deferred_first_visit() {
    const CHILD: u32 = 0x0700;
    let source = Source::load();
    for can_allocate in [false, true] {
        let (mut game, _, id, sounds) = scene();
        let al = &mut game.objs.aliens[id as usize];
        al.sflags = 0;
        al.sflags2 = 0;
        [al.worldx, al.worldy, al.worldz] = [32760, -32760, 1000];
        let available = game.objs.alloc().unwrap();
        while game.objs.alloc().is_some() {}
        if can_allocate {
            game.objs.free(available);
        }
        let mut bus = seed(&source, &game, id);
        seed_inventory(&source, &mut bus, &game);
        source.word(&mut bus, 0, "ALLST", OBJECT as i16);
        if can_allocate {
            source.word(&mut bus, 0, "ALFREELST", CHILD as i16);
        }
        source.run(&mut bus, "BOMWINGDIE_ISTRAT");
        bomwingdie_istrat(&mut game, id);
        let context = format!("special drop can_allocate={can_allocate}");
        compare_pickup_state(&source, &mut bus, &game, id, &context);
        compare_inventory(&source, &mut bus, &game, &context);
        compare_sound(&source, &mut bus, &sounds, &context);
        if !can_allocate {
            assert!(!game
                .objs
                .aliens
                .iter()
                .any(|actor| actor.active && actor.shape == 158));
            continue;
        }
        let active = game.objs.active_indices();
        assert_eq!(
            active[active.iter().position(|&actor| actor == id).unwrap() + 1],
            available
        );
        let drop = game.objs.aliens[available as usize];
        assert_eq!(drop.shape, 158);
        assert_eq!(
            [drop.worldx, drop.worldy, drop.worldz],
            [32760, 32756, 1000]
        );
        assert_eq!(
            drop.sflags2,
            bus.read8(WRAM | (CHILD + source.symbol("AL_SFLAGS2")))
        );
        let installed = WRAM | (CHILD + source.symbol("AL_STRATPTR"));
        assert_eq!(
            u32::from(word(&source, &mut bus, CHILD, "AL_STRATPTR"))
                | u32::from(bus.read8(installed + 2)) << 16,
            source.symbol("ITEM5_ISTRAT")
        );
        source.word(&mut bus, 0, "AL1PT", CHILD as i16);
        source.byte(&mut bus, 0, "ALDEAD", 0);
        game.objs.aldead = 0;
        source.run_at(&mut bus, "ITEM5_ISTRAT", CHILD);
        game.call_strat(drop.stratptr.unwrap(), available);
        let after = game.objs.aliens[available as usize];
        for (name, actual) in [
            ("AL_WORLDX", after.worldx),
            ("AL_WORLDY", after.worldy),
            ("AL_WORLDZ", after.worldz),
        ] {
            assert_eq!(
                actual as u16,
                word(&source, &mut bus, CHILD, name),
                "{context}: {name}"
            );
        }
        assert_eq!(after.worldz, drop.worldz + 20);
        assert_eq!(
            after.sflags2,
            bus.read8(WRAM | (CHILD + source.symbol("AL_SFLAGS2")))
        );
        assert_eq!(
            after.count,
            bus.read8(WRAM | (CHILD + source.symbol("AL_COUNT")))
        );
        compare_inventory(&source, &mut bus, &game, &context);
        compare_sound(&source, &mut bus, &sounds, &context);
    }
}
