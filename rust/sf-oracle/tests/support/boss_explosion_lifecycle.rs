//! Full source strategy passes, from boss death through the last particle.
//!
//! TRANS.dostrats, init_strats_l, Do_strat_l, allocation, effect callbacks and
//! removal execute unmodified. Native Game::run_strategies owns the other side.
//! The map has no pending object (positive map countdown and stationary player).
use super::{Source, WRAM};
use sf_game::{
    alien::{Alien, ExplosionSize, ObjectVisualKind, NUMBER_AL},
    Game,
};
use sf_oracle::{call, call_near, Entry, SnesBus};

fn base(source: &Source, slot: u16) -> u32 {
    source.symbol("ALBLKS") + u32::from(slot) * source.symbol("AL_SIZE")
}

fn word(source: &Source, bus: &mut SnesBus, address: u32, field: &str) -> u16 {
    let address = WRAM | (address + source.symbol(field));
    u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)])
}

fn pointer(source: &Source, bus: &mut SnesBus, address: u32, field: &str, routine: &str) {
    let target = source.symbol(routine);
    for offset in 0..3 {
        bus.write8(
            WRAM | (address + source.symbol(field) + offset),
            (target >> (offset * 8)) as u8,
        );
    }
}

fn initialize(source: &Source, bus: &mut SnesBus, address: u32, actor: &Alien) {
    let returned = call(
        bus,
        source.symbol("INIT_OBJVARS_L"),
        &Entry {
            y: address as u16,
            p: 0x20,
            dbr: 0x7E,
            ..Default::default()
        },
    );
    assert!(returned.returned);
    for (name, value) in [
        ("AL_FLAGS", actor.flags),
        ("AL_HP", actor.hp),
        ("AL_AP", actor.ap),
        ("AL_TYPE", actor.type_),
        ("AL_COLLFLAGS", actor.collflags),
        ("AL_SFLAGS", actor.sflags),
        ("AL_SFLAGS2", actor.sflags2),
        ("AL_SFLAGS3", actor.sflags3),
        ("AL_SFLAGS4", actor.sflags4),
        ("AL_ROTX", actor.rotx),
        ("AL_ROTY", actor.roty),
        ("AL_ROTZ", actor.rotz),
    ] {
        source.byte(bus, address, name, value);
    }
    for (name, value) in [
        ("AL_WORLDX", actor.worldx),
        ("AL_WORLDY", actor.worldy),
        ("AL_WORLDZ", actor.worldz),
        ("AL_VX", actor.vx),
        ("AL_VY", actor.vy),
        ("AL_VZ", actor.vz),
    ] {
        source.word(bus, address, name, value);
    }
    let shape = (0x8000..=u16::MAX)
        .find(|&raw| sf_core::shape::resolve_shape_word(raw) == actor.shape)
        .expect("source shape");
    source.word(bus, address, "AL_SHAPE", shape as i16);
}

fn compare_actor(source: &Source, bus: &mut SnesBus, address: u32, actor: &Alien, context: &str) {
    let shape = word(source, bus, address, "AL_SHAPE");
    match shape {
        0x9570 => assert_eq!(
            actor.visual_kind,
            ObjectVisualKind::ExplosionEnvelope(ExplosionSize::Small),
            "{context}"
        ),
        0x9554 => assert_eq!(
            actor.visual_kind,
            ObjectVisualKind::ExplosionEnvelope(ExplosionSize::Medium),
            "{context}"
        ),
        0x9538 => assert_eq!(
            actor.visual_kind,
            ObjectVisualKind::ExplosionEnvelope(ExplosionSize::Large),
            "{context}"
        ),
        0x951C => assert_eq!(
            actor.visual_kind,
            ObjectVisualKind::ExplosionEnvelope(ExplosionSize::Oversized),
            "{context}"
        ),
        _ => assert_eq!(
            actor.shape,
            sf_core::shape::resolve_shape_word(shape),
            "{context}: shape"
        ),
    }
    let mut translated = *actor;
    translated.shape = shape;
    source.compare_at(bus, address, &translated, context);
    for (field, actual) in [("AL_FLAGS", actor.flags), ("ALX_COLFRAME", actor.colframe)] {
        assert_eq!(
            actual,
            bus.read8(WRAM | (address + source.symbol(field))),
            "{context}: {field}"
        );
    }
}

fn list(source: &Source, bus: &mut SnesBus, name: &str) -> Vec<u16> {
    let mut address = word(source, bus, 0, name);
    let mut result = Vec::new();
    while address != 0 {
        let offset = u32::from(address)
            .checked_sub(source.symbol("ALBLKS"))
            .expect("pool address");
        assert_eq!(offset % source.symbol("AL_SIZE"), 0);
        let slot = (offset / source.symbol("AL_SIZE")) as u16;
        assert!(
            usize::from(slot) < NUMBER_AL && !result.contains(&slot),
            "invalid source list"
        );
        result.push(slot);
        address = word(source, bus, u32::from(address), "_NEXT");
    }
    result
}

#[test]
fn boss_explosion_lifetime_and_pool_reuse_match_original_complete_strategy_passes() {
    let source = Source::load();
    for (routine, native) in [
        (
            "BIGEXPLODE_ISTRAT",
            sf_strat::bosses::boss8_bigexplode as fn(&mut Game, u16),
        ),
        (
            "BOSSEXPLODE_ISTRAT",
            sf_strat::enemy_a::boss1exp_init as fn(&mut Game, u16),
        ),
        (
            "BOSSBIGOUTEXPLODE_ISTRAT",
            sf_strat::enemy_b::bossbigoutexplode_istrat,
        ),
        (
            "BOSSBIGOUTEXPLODEOFF_ISTRAT",
            sf_strat::enemy_b::bossbigoutexplodeoff_istrat,
        ),
    ] {
        for available in [0, 1, 2, 3, 16, NUMBER_AL - 3] {
            // The old regression uses shape 16 (beeanim); the real boss header is 19.
            // Keep both size classes explicit instead of certifying the wrong artwork.
            for shape in [16, 19] {
                for seed in [0u8, 1, 17, 128, 255] {
                    let mut game = Game::new();
                    let player = game.objs.alloc().unwrap();
                    game.vars.internal_playpt = player as i16;
                    game.objs.aliens[player as usize].shape = 2;
                    game.objs.aliens[player as usize].hp = 40;
                    game.objs.aliens[player as usize].sflags4 = sf_game::alien::ASF4_PLAYEROBJ;
                    game.objs.aliens[player as usize].worldz = 5000;
                    let dummy = game.create_player_dummy().unwrap();
                    let boss = game.objs.alloc().unwrap();
                    let death = game.world.register_strategy(native);
                    let actor = &mut game.objs.aliens[boss as usize];
                    actor.shape = shape;
                    actor.flags = sf_game::draw::AF_INVIEW_PL;
                    actor.hp = 0;
                    actor.ap = 10;
                    actor.type_ |= sf_game::alien::ATGND;
                    actor.sflags2 = sf_game::alien::ASF2_COLLDISABLE;
                    [actor.worldx, actor.worldy, actor.worldz] = [0, -70, 7215];
                    actor.roty = 128;
                    actor.rotz = 120;
                    if routine.ends_with("OFF_ISTRAT") {
                        [actor.vx, actor.vy, actor.vz] = [-65, 90, -18];
                    }
                    actor.expstratptr = Some(death);
                    let actor = *actor;
                    game.vars.gameframe = 70;
                    game.vars.pviewvelz = 65;
                    game.vars.rng = [
                        seed,
                        seed.wrapping_add(83),
                        seed ^ 0xA5,
                        seed.wrapping_mul(71),
                    ];
                    while game.objs.active_indices().len() < NUMBER_AL - available {
                        let filler = game.objs.alloc().unwrap();
                        game.objs.aliens[filler as usize].hp = 1;
                    }
                    let mut bus = SnesBus::new(source.rom.clone());
                    source.seed(&mut bus, &actor, &game);
                    for slot in game.objs.active_indices() {
                        initialize(
                            &source,
                            &mut bus,
                            base(&source, slot),
                            &game.objs.aliens[slot as usize],
                        );
                    }
                    pointer(
                        &source,
                        &mut bus,
                        base(&source, boss),
                        "ALX_EXPSTRATPTR",
                        routine,
                    );
                    if routine == "BOSSEXPLODE_ISTRAT" {
                        pointer(
                            &source,
                            &mut bus,
                            base(&source, boss),
                            "ALX_TEMPSTRATPTR",
                            "BOSS1EXP_ISTRAT",
                        );
                    }
                    let active = game.objs.active_indices();
                    for (name, order) in [
                        ("ALLST", active),
                        (
                            "ALFREELST",
                            ((NUMBER_AL - available) as u16..NUMBER_AL as u16).collect(),
                        ),
                    ] {
                        source.word(
                            &mut bus,
                            0,
                            name,
                            order.first().map_or(0, |&slot| base(&source, slot) as i16),
                        );
                        for (position, &slot) in order.iter().enumerate() {
                            source.word(
                                &mut bus,
                                base(&source, slot),
                                "_PREV",
                                if position == 0 {
                                    0
                                } else {
                                    base(&source, order[position - 1]) as i16
                                },
                            );
                            source.word(
                                &mut bus,
                                base(&source, slot),
                                "_NEXT",
                                order
                                    .get(position + 1)
                                    .map_or(0, |&next| base(&source, next) as i16),
                            );
                        }
                    }
                    for (name, value) in [
                        ("PLAYPT", base(&source, player) as i16),
                        ("INTERNALPLAYPT", base(&source, player) as i16),
                        ("DUMMYOBJ", base(&source, dummy) as i16),
                        ("LASTPLAYZ", 5000),
                        ("MAPCNT", 32767),
                        ("GAMEFRAME", game.vars.gameframe as i16),
                    ] {
                        source.word(&mut bus, 0, name, value);
                    }
                    for (offset, value) in game.vars.rng.into_iter().enumerate() {
                        bus.write8(WRAM | (source.symbol("RAND") + offset as u32), value);
                    }
                    for visit in 0..160 {
                        let result = call_near(
                            &mut bus,
                            source.symbol("DOSTRATS"),
                            &Entry {
                                p: 0x20,
                                ..Default::default()
                            },
                        );
                        assert!(
                            result.returned,
                            "source strategy pass seed={seed} visit={visit}"
                        );
                        game.run_strategies();
                        let context = format!("{routine} available={available} shape={shape} seed={seed} visit={visit}");
                        assert_eq!(
                            game.objs.active_indices(),
                            list(&source, &mut bus, "ALLST"),
                            "{context}: active order"
                        );
                        let mut free = Vec::new();
                        let mut slot = game.objs.free_head;
                        while let Some(id) = slot {
                            free.push(id);
                            slot = game.objs.aliens[id as usize].next;
                        }
                        assert_eq!(
                            free,
                            list(&source, &mut bus, "ALFREELST"),
                            "{context}: free order"
                        );
                        for slot in game.objs.active_indices() {
                            compare_actor(
                                &source,
                                &mut bus,
                                base(&source, slot),
                                &game.objs.aliens[slot as usize],
                                &format!("{context} slot={slot}"),
                            );
                        }
                        for (offset, value) in game.vars.rng.into_iter().enumerate() {
                            assert_eq!(
                                value,
                                bus.read8(WRAM | (source.symbol("RAND") + offset as u32)),
                                "{context}: random byte {offset}"
                            );
                        }
                        assert_eq!(
                            game.vars.gameflags,
                            bus.read8(WRAM | source.symbol("GAMEFLAGS")),
                            "{context}: game flags"
                        );
                    }
                }
            }
        }
    }
}
