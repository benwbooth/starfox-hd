//! Original instructions own boss-circle allocation, entry and child scheduling.
use super::{repair_chain::compare_sound, repair_chain::Sounds, Source, OBJECT, WRAM};
use sf_core::screen_fill_circle::{ScreenFillCircleCenter, ScreenFillCirclePhase};
use sf_game::{alien::Alien, Game};
use sf_oracle::SnesBus;
use sf_strat::enemy_a::{circdelayexplode_init, circdelayexplode_strat, strat_qboss_explode_init};

const CHILDREN: [u32; 2] = [0x0700, 0x0900];

fn word(source: &Source, bus: &mut SnesBus, base: u32, name: &str) -> u16 {
    let address = WRAM | (base + source.symbol(name));
    u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)])
}

fn strategy(source: &Source, bus: &mut SnesBus, base: u32, name: &str) -> u32 {
    let address = WRAM | (base + source.symbol(name));
    u32::from(bus.read8(address))
        | u32::from(bus.read8(address + 1)) << 8
        | u32::from(bus.read8(address + 2)) << 16
}

fn compare_child(source: &Source, bus: &mut SnesBus, base: u32, al: &Alien, context: &str) {
    let mut translated = *al;
    let shape = word(source, bus, base, "AL_SHAPE");
    assert_eq!(
        sf_core::shape::resolve_shape_word(shape),
        al.shape,
        "{context}: shape"
    );
    translated.shape = shape;
    source.compare_at(bus, base, &translated, context);
    assert_eq!(
        al.flags,
        bus.read8(WRAM | (base + source.symbol("AL_FLAGS"))),
        "{context}: flags"
    );
}

#[test]
fn boss_circle_entries_match_original_all_timer_and_flag_bytes_with_zero_one_or_two_free_slots() {
    let source = Source::load();
    for (name, native) in [
        (
            "CIRCDELAYEXPLODE_ISTRAT",
            circdelayexplode_init as fn(&mut Game, u16),
        ),
        ("CIRCDELAYEXPLODE_STRAT", circdelayexplode_strat),
        ("QBOSSEXPLODE_ISTRAT", strat_qboss_explode_init),
    ] {
        for free in 0..=2 {
            for flags_axis in [false, true] {
                for byte in 0..=u8::MAX {
                    let sounds = Sounds::default();
                    let mut game = Game::with_hooks(Box::new(sounds.clone()));
                    let id = game.objs.alloc().unwrap();
                    while game.objs.active_indices().len() < game.objs.aliens.len() - free {
                        game.objs.alloc().unwrap();
                    }
                    let initial_free = game.objs.free_head;
                    game.vars.pviewvelz = if byte & 1 == 0 { 65 } else { -65 };
                    let object = &mut game.objs.aliens[id as usize];
                    [object.worldx, object.worldy, object.worldz] = [32760, -32760, 32760];
                    [object.rotx, object.roty, object.rotz] = [251, 252, 253];
                    object.count = if flags_axis { 0 } else { byte };
                    let flags = if flags_axis { byte } else { 0xFF };
                    [
                        object.sflags,
                        object.sflags2,
                        object.sflags3,
                        object.sflags4,
                    ] = [flags; 4];
                    object.hp = 17;
                    object.ap = 23;
                    let object = *object;
                    let mut bus = SnesBus::new(source.rom.clone());
                    source.seed(&mut bus, &object, &game);
                    source.byte(&mut bus, OBJECT, "AL_FLAGS", object.flags);
                    source.word(&mut bus, 0, "ALLST", OBJECT as i16);
                    source.word(
                        &mut bus,
                        0,
                        "ALFREELST",
                        if free > 0 { CHILDREN[0] as i16 } else { 0 },
                    );
                    if free == 2 {
                        // Original list links, not an allocator replacement.
                        source.word(&mut bus, CHILDREN[0], "_NEXT", CHILDREN[1] as i16);
                    }
                    source.run(&mut bus, name);
                    native(&mut game, id);
                    let context = format!("{name} free={free} flags_axis={flags_axis} byte={byte}");
                    source.compare(&mut bus, &game.objs.aliens[id as usize], &context);
                    assert_eq!(
                        game.objs.aldead,
                        bus.read8(WRAM | source.symbol("ALDEAD")),
                        "{context}: removal"
                    );
                    assert_eq!(
                        game.vars.gameflags,
                        bus.read8(WRAM | source.symbol("GAMEFLAGS")),
                        "{context}: game flags"
                    );
                    compare_sound(&source, &mut bus, &sounds, &context);
                    let circle = word(&source, &mut bus, 0, "CIRCLEANIM");
                    if circle == 0 {
                        assert!(!game.vars.screen_fill_circle.is_active(), "{context}");
                        assert_eq!(
                            game.objs.free_head, initial_free,
                            "{context}: no allocation"
                        );
                        continue;
                    }
                    assert_eq!(circle, source.symbol("FILLSCREEN_CIRCLE") as u16);
                    assert_eq!(
                        game.vars.screen_fill_circle.phase,
                        ScreenFillCirclePhase::BossExpanding
                    );
                    if free == 0 {
                        assert_eq!(
                            game.vars.screen_fill_circle.center,
                            ScreenFillCircleCenter::Screen
                        );
                        continue;
                    }
                    let anchor = initial_free.unwrap();
                    assert_eq!(
                        game.vars.screen_fill_circle.center,
                        ScreenFillCircleCenter::Object(anchor + 1)
                    );
                    assert_eq!(word(&source, &mut bus, 0, "CIRCLEOBJ"), CHILDREN[0] as u16);
                    compare_child(
                        &source,
                        &mut bus,
                        CHILDREN[0],
                        &game.objs.aliens[anchor as usize],
                        &context,
                    );
                    assert_eq!(
                        strategy(&source, &mut bus, CHILDREN[0], "AL_STRATPTR"),
                        source.symbol("STAYREL_STRAT")
                    );
                    let particle_created =
                        free == 2 && word(&source, &mut bus, 0, "ALFREELST") == 0;
                    let first_child = game.objs.aliens[id as usize].next.unwrap();
                    if particle_created {
                        assert_eq!(first_child, anchor + 1, "{context}: insert after parent");
                        assert_eq!(word(&source, &mut bus, OBJECT, "_NEXT"), CHILDREN[1] as u16);
                        assert_eq!(game.objs.aliens[first_child as usize].next, Some(anchor));
                        compare_child(
                            &source,
                            &mut bus,
                            CHILDREN[1],
                            &game.objs.aliens[first_child as usize],
                            &context,
                        );
                        assert_eq!(
                            strategy(&source, &mut bus, CHILDREN[1], "AL_STRATPTR"),
                            source.symbol("BIGPARTICLEEXPLODE_ISTRAT")
                        );
                        let init = game.objs.aliens[first_child as usize].stratptr.unwrap();
                        source.run_at(&mut bus, "BIGPARTICLEEXPLODE_ISTRAT", CHILDREN[1]);
                        game.call_strat(init, first_child);
                        compare_child(
                            &source,
                            &mut bus,
                            CHILDREN[1],
                            &game.objs.aliens[first_child as usize],
                            &context,
                        );
                    } else {
                        assert_eq!(first_child, anchor, "{context}: only anchor allocated");
                        assert_eq!(word(&source, &mut bus, OBJECT, "_NEXT"), CHILDREN[0] as u16);
                    }
                }
            }
        }
    }
}

#[test]
fn circle_removal_releases_fire_only_after_both_child_allocations_have_failed() {
    use sf_game::alien::AFONFIRE;
    const FIRE: u32 = 0x0B00;
    let source = Source::load();
    for linked in [false, true] {
        for on_fire in [false, true] {
            for removal_count in [0, 1, 255] {
                let mut game = Game::new();
                let id = game.objs.alloc().unwrap();
                let fire = game.objs.alloc().unwrap();
                while game.objs.alloc().is_some() {}
                game.objs.active_move_after(fire, id);
                game.objs.aldead = removal_count;
                let object = &mut game.objs.aliens[id as usize];
                object.flags = if on_fire { AFONFIRE } else { 0 };
                object.fireobjptr = if linked { fire + 1 } else { 0 };
                object.sflags2 = sf_strat::enemy_a::ASF2_SFLAG1;
                let object = *object;
                let mut bus = SnesBus::new(source.rom.clone());
                source.seed(&mut bus, &object, &game);
                source.byte(&mut bus, OBJECT, "AL_FLAGS", object.flags);
                source.word(
                    &mut bus,
                    OBJECT,
                    "ALX_FIREOBJPTR",
                    if linked { FIRE as i16 } else { 0 },
                );
                source.word(&mut bus, OBJECT, "_NEXT", FIRE as i16);
                source.word(&mut bus, FIRE, "_PREV", OBJECT as i16);
                source.word(&mut bus, 0, "ALLST", OBJECT as i16);
                source.byte(&mut bus, 0, "ALDEAD", removal_count);
                source.run(&mut bus, "CIRCDELAYEXPLODE_STRAT");
                circdelayexplode_strat(&mut game, id);
                let context =
                    format!("linked={linked} on_fire={on_fire} removal_count={removal_count}");
                source.compare(&mut bus, &game.objs.aliens[id as usize], &context);
                assert_eq!(game.objs.aldead, bus.read8(WRAM | source.symbol("ALDEAD")));
                assert_eq!(
                    game.objs.aliens[id as usize].flags,
                    bus.read8(WRAM | (OBJECT + source.symbol("AL_FLAGS")))
                );
                let removed = word(&source, &mut bus, 0, "ALFREELST") == FIRE as u16;
                assert_eq!(removed, linked && on_fire);
                assert_eq!(!game.objs.aliens[fire as usize].active, removed);
                assert_eq!(
                    game.vars.screen_fill_circle.center,
                    ScreenFillCircleCenter::Screen
                );
                assert_eq!(game.objs.free_head, removed.then_some(fire));
                assert_eq!(word(&source, &mut bus, 0, "CIRCLEOBJ"), 0);
                assert_eq!(
                    game.objs.aliens[id as usize].fireobjptr == 0,
                    word(&source, &mut bus, OBJECT, "ALX_FIREOBJPTR") == 0
                );
            }
        }
    }
}

#[test]
fn particle_child_lifetime_and_scroll_match_original_on_every_subsequent_visit() {
    use sf_strat::enemy_a::{bigparticleexplode_istrat, bigparticleexplode_strat};
    let source = Source::load();
    for initial in 0..=u8::MAX {
        for relative in [false, true] {
            let mut game = Game::new();
            let id = game.objs.alloc().unwrap();
            game.vars.pviewvelz = -65;
            let object = &mut game.objs.aliens[id as usize];
            object.count = initial;
            object.worldz = -32760;
            object.sflags4 = if relative {
                sf_game::alien::ASF4_RELEXPLODE
            } else {
                0
            };
            let object = *object;
            let mut bus = SnesBus::new(source.rom.clone());
            source.seed(&mut bus, &object, &game);
            source.run(&mut bus, "BIGPARTICLEEXPLODE_ISTRAT");
            bigparticleexplode_istrat(&mut game, id);
            for visit in 1..=256 {
                source.run(&mut bus, "BIGPARTICLEEXPLODE_STRAT");
                bigparticleexplode_strat(&mut game, id);
                let context = format!("initial={initial} relative={relative} visit={visit}");
                source.compare(&mut bus, &game.objs.aliens[id as usize], &context);
                assert_eq!(
                    game.objs.aldead,
                    bus.read8(WRAM | source.symbol("ALDEAD")),
                    "{context}"
                );
                if game.objs.aldead != 0 {
                    break;
                }
                assert!(
                    visit < 256,
                    "source emitter must expire within one counter wrap"
                );
            }
        }
    }
}

#[test]
fn outward_boss_explosion_schedules_both_children_with_all_four_inherited_flag_bytes() {
    use sf_strat::enemy_b::{bossbigoutexplode_istrat, bossbigoutexplodeoff_istrat};
    const DUMMY: u32 = 0x0B00;
    let source = Source::load();
    for (name, native, offset) in [
        (
            "BOSSBIGOUTEXPLODE_ISTRAT",
            bossbigoutexplode_istrat as fn(&mut Game, u16),
            false,
        ),
        (
            "BOSSBIGOUTEXPLODEOFF_ISTRAT",
            bossbigoutexplodeoff_istrat,
            true,
        ),
    ] {
        for flags in 0..=u8::MAX {
            let mut game = Game::new();
            let id = game.objs.alloc().unwrap();
            while game.objs.active_indices().len() < game.objs.aliens.len() - 2 {
                game.objs.alloc().unwrap();
            }
            let first_free = game.objs.free_head.unwrap();
            let object = &mut game.objs.aliens[id as usize];
            [object.worldx, object.worldy, object.worldz] = [32760, -32760, 32760];
            [object.vx, object.vy, object.vz] = [37, -71, 99];
            [
                object.sflags,
                object.sflags2,
                object.sflags3,
                object.sflags4,
            ] = [
                flags,
                flags ^ 0xA5,
                flags.rotate_left(1),
                flags.rotate_right(1),
            ];
            let object = *object;
            let mut bus = SnesBus::new(source.rom.clone());
            source.seed(&mut bus, &object, &game);
            source.word(&mut bus, 0, "ALLST", OBJECT as i16);
            source.word(&mut bus, 0, "ALFREELST", CHILDREN[0] as i16);
            source.word(&mut bus, CHILDREN[0], "_NEXT", CHILDREN[1] as i16);
            source.word(&mut bus, 0, "DUMMYOBJ", DUMMY as i16);
            source.run(&mut bus, name);
            native(&mut game, id);
            let context = format!("{name} offset={offset} flags={flags}");
            for (slot, base, expected_strategy) in [
                (first_free, CHILDREN[0], "CIRCDELAYEXPLODE_ISTRAT"),
                (first_free + 1, CHILDREN[1], "BIGPARTICLEEXPLODE_ISTRAT"),
            ] {
                compare_child(
                    &source,
                    &mut bus,
                    base,
                    &game.objs.aliens[slot as usize],
                    &context,
                );
                assert_eq!(
                    strategy(&source, &mut bus, base, "AL_STRATPTR"),
                    source.symbol(expected_strategy)
                );
            }
            assert_eq!(game.objs.aliens[id as usize].next, Some(first_free + 1));
            assert_eq!(
                game.objs.aliens[(first_free + 1) as usize].next,
                Some(first_free)
            );
            // This gate covers the constructors, not the separately retained
            // outward-sprite dummy-object RNG and boss delay-removal paths.
        }
    }
}
