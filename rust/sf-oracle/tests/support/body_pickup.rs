//! Body health and configured laser drops, executed from the original entries.
use super::{
    compare_pickup_state,
    repair_chain::{compare_sound, scene, seed},
    Source, OBJECT, WRAM,
};
use sf_core::player_view::PlayerViewMode;
use sf_game::Game;
use sf_oracle::SnesBus;
use sf_strat::enemies_ground::{item3_istrat, winglazermandie_istrat, wldie_istrat};

const BODY: u32 = 0x0B00;
const CHILD: u32 = 0x0700;

fn seed_body(source: &Source, bus: &mut SnesBus, hp: u8) {
    source.word(bus, 0, "PCBOXOBJ_B", BODY as i16);
    source.byte(bus, BODY, "AL_HP", hp);
}

fn word(source: &Source, bus: &mut SnesBus, base: u32, name: &str) -> u16 {
    let address = WRAM | (base + source.symbol(name));
    u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)])
}

#[test]
fn body_pickup_matches_original_every_health_byte_and_death_cockpit_order() {
    let source = Source::load();
    for hp in 0..=u8::MAX {
        for dead in [false, true] {
            for cockpit in [false, true] {
                let (mut game, _, id, sounds) = scene();
                let body = game.objs.alloc().unwrap();
                game.coldet.pcbox.body = Some(body);
                game.objs.aliens[body as usize].hp = hp;
                // This stale import mirror deliberately names another actor.
                game.vars.strategy.player_collision_objects[0] = 0;
                game.objs.aliens[0].hp = 23;
                game.vars.pshipflags2 = if dead { 128 } else { 0 };
                if cockpit {
                    game.vars.player_view_mode = PlayerViewMode::Cockpit;
                }
                let mut bus = seed(&source, &game, id);
                seed_body(&source, &mut bus, hp);
                if cockpit {
                    source.byte(
                        &mut bus,
                        0,
                        "SPLAYERFLYMODE",
                        source.symbol("SPFM_INSIDE") as u8,
                    );
                }
                source.run(&mut bus, "ITEM3_ISTRAT");
                item3_istrat(&mut game, id);
                let context = format!("body hp={hp} dead={dead} cockpit={cockpit}");
                compare_pickup_state(&source, &mut bus, &game, id, &context);
                compare_sound(&source, &mut bus, &sounds, &context);
                assert_eq!(
                    game.objs.aliens[body as usize].hp,
                    bus.read8(WRAM | (BODY + source.symbol("AL_HP"))),
                    "{context}: health"
                );
                assert_eq!(
                    game.objs.aliens[0].hp, 23,
                    "{context}: stale body mirror untouched"
                );
            }
        }
    }
}

#[test]
fn body_pickup_matches_original_wrapped_range_spin_and_unconditional_drift() {
    let source = Source::load();
    let edges = [
        -32768i16, -32760, -121, -120, -119, -61, -60, -59, -1, 0, 1, 59, 60, 61, 119, 120, 121,
        32760, 32767,
    ];
    for axis in 0..3 {
        for value in edges {
            for reference in [-32760i16, 0, 32760] {
                for (drift, death) in [(0, 0), (1, 0), (0, 128), (1, 128)] {
                    let (mut game, player, id, sounds) = scene();
                    let body = game.objs.alloc().unwrap();
                    game.coldet.pcbox.body = Some(body);
                    game.objs.aliens[body as usize].hp = 11;
                    let mut position = [0; 3];
                    position[axis] = reference;
                    let pl = &mut game.objs.aliens[player as usize];
                    [pl.worldx, pl.worldy, pl.worldz] = position;
                    position[axis] = value;
                    let al = &mut game.objs.aliens[id as usize];
                    [al.worldx, al.worldy, al.worldz] = position;
                    [al.roty, al.rotz] = [254, 255];
                    al.sbyte1 = drift;
                    game.vars.pshipflags2 = death;
                    let mut bus = seed(&source, &game, id);
                    seed_body(&source, &mut bus, 11);
                    source.run(&mut bus, "ITEM3_ISTRAT");
                    item3_istrat(&mut game, id);
                    let context = format!("body axis={axis} coordinate={value} reference={reference} drift={drift} death={death}");
                    compare_pickup_state(&source, &mut bus, &game, id, &context);
                    compare_sound(&source, &mut bus, &sounds, &context);
                    assert_eq!(
                        game.objs.aliens[body as usize].hp,
                        bus.read8(WRAM | (BODY + source.symbol("AL_HP"))),
                        "{context}: health"
                    );
                }
            }
        }
    }
}

#[test]
fn laser_enemy_drops_match_original_flag_branches_allocation_and_deferred_entry() {
    let source = Source::load();
    for (name, native) in [
        (
            "WINGLAZERMANDIE_ISTRAT",
            winglazermandie_istrat as fn(&mut Game, u16),
        ),
        ("WLDIE_ISTRAT", wldie_istrat),
    ] {
        for flags in 0..=u8::MAX {
            for upgrades in [0, 16, 239, 255] {
                for can_allocate in [false, true] {
                    let (mut game, _, id, sounds) = scene();
                    let al = &mut game.objs.aliens[id as usize];
                    al.sflags = 0;
                    al.sflags2 = 0;
                    [al.worldx, al.worldy, al.worldz] = [32760, -32760, 1000];
                    game.vars.pshipflags = flags;
                    game.vars.pshipflags3 = upgrades;
                    let available = game.objs.alloc().unwrap();
                    while game.objs.alloc().is_some() {}
                    if can_allocate {
                        game.objs.free(available);
                    }
                    let mut bus = seed(&source, &game, id);
                    source.word(&mut bus, 0, "ALLST", OBJECT as i16);
                    if can_allocate {
                        source.word(&mut bus, 0, "ALFREELST", CHILD as i16);
                    }
                    source.run(&mut bus, name);
                    native(&mut game, id);
                    let context = format!(
                        "{name} flags={flags} upgrades={upgrades} can_allocate={can_allocate}"
                    );
                    compare_pickup_state(&source, &mut bus, &game, id, &context);
                    compare_sound(&source, &mut bus, &sounds, &context);
                    let installed = u32::from(word(&source, &mut bus, CHILD, "AL_STRATPTR"))
                        | u32::from(bus.read8(WRAM | (CHILD + source.symbol("AL_STRATPTR") + 2)))
                            << 16;
                    let native_drop = game
                        .objs
                        .aliens
                        .iter()
                        .position(|al| al.active && al.shape == 160);
                    if installed == 0 {
                        assert_eq!(native_drop, None, "{context}: no drop");
                        assert_eq!(
                            game.objs.aliens[available as usize].active, !can_allocate,
                            "{context}: no substitute helper or allocation"
                        );
                        continue;
                    }
                    assert_eq!(
                        installed,
                        source.symbol("ITEM7_ISTRAT"),
                        "{context}: deferred initializer"
                    );
                    assert_eq!(
                        native_drop,
                        Some(available as usize),
                        "{context}: allocation"
                    );
                    let active = game.objs.active_indices();
                    assert_eq!(
                        active[active.iter().position(|&actor| actor == id).unwrap() + 1],
                        available
                    );
                    let drop = game.objs.aliens[available as usize];
                    assert_eq!(
                        [drop.worldx, drop.worldy, drop.worldz],
                        [32760, -32760, 1000]
                    );
                    assert_eq!(
                        drop.sflags2,
                        bus.read8(WRAM | (CHILD + source.symbol("AL_SFLAGS2")))
                    );
                    source.word(&mut bus, 0, "AL1PT", CHILD as i16);
                    source.byte(&mut bus, 0, "ALDEAD", 0);
                    game.objs.aldead = 0;
                    source.run_at(&mut bus, "ITEM7_ISTRAT", CHILD);
                    game.call_strat(drop.stratptr.unwrap(), available);
                    let after = game.objs.aliens[available as usize];
                    for (field, actual) in [
                        ("AL_WORLDX", after.worldx),
                        ("AL_WORLDY", after.worldy),
                        ("AL_WORLDZ", after.worldz),
                    ] {
                        assert_eq!(
                            actual as u16,
                            word(&source, &mut bus, CHILD, field),
                            "{context}: {field}"
                        );
                    }
                    for (field, actual) in [
                        ("AL_SFLAGS2", after.sflags2),
                        ("AL_ROTY", after.roty),
                        ("AL_ROTZ", after.rotz),
                        ("AL_COUNT", after.count),
                    ] {
                        assert_eq!(
                            actual,
                            bus.read8(WRAM | (CHILD + source.symbol(field))),
                            "{context}: {field}"
                        );
                    }
                    compare_sound(&source, &mut bus, &sounds, &context);
                }
            }
        }
    }
}
