//! Delay entry, stored countdown bytes and the real fallback actor.
use super::{Source, OBJECT, WRAM};
use sf_game::{
    alien::{ExplosionSize, ObjectVisualKind},
    Game,
};
use sf_oracle::SnesBus;
use sf_strat::enemy_a::{
    bossdelayexplode_strat, delayexplode_init, delayexplode_strat, delayremove_istrat,
    delayremove_strat, delayremoverel_istrat, strat_boss_delay_explode_init, strat_explode,
};
use sf_strat::enemy_a::{set_bossflags, strat_boss_explode_init, BF_DYING};
use sf_strat::enemy_b::{bossbigoutexplode_istrat, bossbigoutexplodeoff_istrat};
use sf_strat::theend::theend_flyaway_strat;

const CHILD: u32 = 0x0700;
const DUMMY: u32 = 0x0B00;

#[test]
fn explosion_removes_attached_fire_before_allocating_the_sprite_even_when_full() {
    use sf_game::alien::{AFONFIRE, ASF3_NOPOLYEXP, ASF_HITFLASH, ASF_SHADOW, ASF_SSPRITE};
    use sf_strat::enemy_a::ASF2_NOEXPSND;
    let source = Source::load();
    for in_view in [false, true] {
        for flagged in [false, true] {
            for linked in [false, true] {
                for free in [false, true] {
                    for no_polygons in [false, true] {
                        for removal in [0u8, 1, 255] {
                            let mut game = Game::new();
                            let id = game.objs.alloc().unwrap();
                            let fire = game.objs.alloc().unwrap();
                            game.objs.active_move_after(fire, id);
                            while game.objs.active_indices().len()
                                < game.objs.aliens.len() - usize::from(free)
                            {
                                game.objs.alloc().unwrap();
                            }
                            let next_free = game.objs.free_head;
                            game.objs.aldead = removal;
                            let object = &mut game.objs.aliens[id as usize];
                            object.shape = 16;
                            object.hp = 19;
                            object.sflags = ASF_HITFLASH | ASF_SHADOW | ASF_SSPRITE;
                            object.sflags2 = ASF2_NOEXPSND;
                            object.sflags3 = if no_polygons { ASF3_NOPOLYEXP } else { 0 };
                            object.flags = if in_view {
                                sf_game::draw::AF_INVIEW_PL
                            } else {
                                0
                            } | if flagged { AFONFIRE } else { 0 };
                            object.fireobjptr = if linked { fire + 1 } else { 0 };
                            let object = *object;
                            let mut bus = SnesBus::new(source.rom.clone());
                            source.seed(&mut bus, &object, &game);
                            source.word(&mut bus, OBJECT, "AL_SHAPE", 0x9CC4u16 as i16); // beeanim, native 16
                            source.byte(&mut bus, OBJECT, "AL_FLAGS", object.flags);
                            source.word(
                                &mut bus,
                                OBJECT,
                                "ALX_FIREOBJPTR",
                                if linked { DUMMY as i16 } else { 0 },
                            );
                            source.byte(&mut bus, 0, "ALDEAD", removal);
                            source.word(&mut bus, 0, "ALLST", OBJECT as i16);
                            source.word(&mut bus, OBJECT, "_NEXT", DUMMY as i16);
                            source.word(&mut bus, DUMMY, "_PREV", OBJECT as i16);
                            source.word(
                                &mut bus,
                                0,
                                "ALFREELST",
                                if free { CHILD as i16 } else { 0 },
                            );
                            source.byte(
                                &mut bus,
                                DUMMY,
                                "AL_COLLFLAGS",
                                game.objs.aliens[fire as usize].collflags,
                            );
                            source.run(&mut bus, "EXPLODE_ISTRAT");
                            strat_explode(&mut game, id);
                            let context = format!("view={in_view} flagged={flagged} linked={linked} free={free} no_poly={no_polygons} removal={removal}");
                            let mut translated = game.objs.aliens[id as usize];
                            let address = WRAM | (OBJECT + source.symbol("AL_SHAPE"));
                            let shape =
                                u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
                            assert_eq!(
                                translated.shape,
                                sf_core::shape::resolve_shape_word(shape),
                                "{context}: shape"
                            );
                            translated.shape = shape;
                            source.compare(&mut bus, &translated, &context);
                            assert_eq!(
                                translated.flags,
                                bus.read8(WRAM | (OBJECT + source.symbol("AL_FLAGS"))),
                                "{context}: flags"
                            );
                            assert_eq!(
                                game.objs.aldead,
                                bus.read8(WRAM | source.symbol("ALDEAD")),
                                "{context}: removal marker"
                            );
                            assert_eq!(
                                translated.fireobjptr != 0,
                                linked && !flagged,
                                "{context}: retained fire link"
                            );
                            let creates_sprite = in_view && (free || (flagged && linked));
                            if creates_sprite {
                                let (slot, base) = if flagged && linked {
                                    (fire, DUMMY)
                                } else {
                                    (next_free.unwrap(), CHILD)
                                };
                                let mut sprite = game.objs.aliens[slot as usize];
                                assert!(sprite.active, "{context}: reclaimed fire slot");
                                let address = WRAM | (base + source.symbol("AL_SHAPE"));
                                let shape = u16::from_le_bytes([
                                    bus.read8(address),
                                    bus.read8(address + 1),
                                ]);
                                assert_eq!(
                                    sprite.shape,
                                    sf_core::shape::resolve_shape_word(shape),
                                    "{context}: sprite shape"
                                );
                                sprite.shape = shape;
                                source.compare_at(&mut bus, base, &sprite, &context);
                            } else {
                                assert_eq!(
                                    game.objs.aliens[fire as usize].active,
                                    !(flagged && linked),
                                    "{context}: attachment lifetime"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn exhausted_boss_barrages_preserve_fallback_mutations_and_every_random_draw() {
    let source = Source::load();
    for (name, native, size) in [
        (
            "BOSSEXPLODE_ISTRAT",
            strat_boss_explode_init as fn(&mut Game, u16),
            ExplosionSize::Large,
        ),
        (
            "BOSSBIGOUTEXPLODE_ISTRAT",
            bossbigoutexplode_istrat,
            ExplosionSize::Oversized,
        ),
        (
            "BOSSBIGOUTEXPLODEOFF_ISTRAT",
            bossbigoutexplodeoff_istrat,
            ExplosionSize::Oversized,
        ),
    ] {
        for seed in 0..=u8::MAX {
            let mut game = Game::new();
            let id = game.objs.alloc().unwrap();
            let dummy = game.objs.alloc().unwrap();
            game.vars.dummyobj = dummy as i16;
            while game.objs.free_head.is_some() {
                game.objs.alloc().unwrap();
            }
            game.vars.pviewvelz = 65;
            game.vars.rng = [
                seed,
                seed.wrapping_add(83),
                seed ^ 0xA5,
                seed.wrapping_mul(71),
            ];
            set_bossflags(&mut game, BF_DYING);
            let object = &mut game.objs.aliens[id as usize];
            [object.worldx, object.worldy, object.worldz] = [32760, -32760, 1234];
            [object.vx, object.vy, object.vz] = [-231, 75, -143];
            object.sflags = seed;
            object.sflags2 = seed.rotate_left(2);
            object.sflags3 = seed.rotate_left(4);
            object.sflags4 = seed.rotate_left(6);
            let object = *object;
            let mut bus = SnesBus::new(source.rom.clone());
            source.seed(&mut bus, &object, &game);
            source.word(&mut bus, 0, "DUMMYOBJ", DUMMY as i16);
            source.word(&mut bus, 0, "ALLST", OBJECT as i16);
            source.byte(&mut bus, 0, "BOSSFLAGS", BF_DYING);
            for (offset, value) in game.vars.rng.into_iter().enumerate() {
                bus.write8(WRAM | (source.symbol("RAND") + offset as u32), value);
            }
            let fallback = &mut game.objs.aliens[dummy as usize];
            [fallback.worldx, fallback.worldy, fallback.worldz] = [-32760, 32760, 30000];
            [fallback.vx, fallback.vy, fallback.vz] = [71, -94, 127];
            fallback.hp = 31;
            fallback.ap = 17;
            fallback.sflags = seed;
            fallback.sflags2 = seed.rotate_left(1);
            fallback.sflags3 = seed.rotate_left(2);
            fallback.sflags4 = seed.rotate_left(3);
            for (field, value) in [
                ("AL_HP", fallback.hp),
                ("AL_AP", fallback.ap),
                ("AL_SFLAGS", fallback.sflags),
                ("AL_SFLAGS2", fallback.sflags2),
                ("AL_SFLAGS3", fallback.sflags3),
                ("AL_SFLAGS4", fallback.sflags4),
                ("AL_COLLFLAGS", fallback.collflags),
            ] {
                source.byte(&mut bus, DUMMY, field, value);
            }
            for (field, value) in [
                ("AL_WORLDX", fallback.worldx),
                ("AL_WORLDY", fallback.worldy),
                ("AL_WORLDZ", fallback.worldz),
                ("AL_VX", fallback.vx),
                ("AL_VY", fallback.vy),
                ("AL_VZ", fallback.vz),
            ] {
                source.word(&mut bus, DUMMY, field, value);
            }
            source.run(&mut bus, name);
            native(&mut game, id);
            let context = format!("{name} exhausted seed={seed}");
            source.compare(&mut bus, &game.objs.aliens[id as usize], &context);
            let mut actual = game.objs.aliens[dummy as usize];
            assert_eq!(
                actual.visual_kind,
                ObjectVisualKind::ExplosionEnvelope(size),
                "{context}"
            );
            let address = WRAM | (DUMMY + source.symbol("AL_SHAPE"));
            actual.shape = u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
            source.compare_at(&mut bus, DUMMY, &actual, &context);
            assert!(
                actual.stratptr.is_none(),
                "failed construction must not replace fallback strategy"
            );
            for (offset, value) in game.vars.rng.into_iter().enumerate() {
                assert_eq!(
                    value,
                    bus.read8(WRAM | (source.symbol("RAND") + offset as u32)),
                    "{context}: random byte {offset}"
                );
            }
            assert!(game.objs.free_head.is_none());
        }
    }
}

fn install(source: &Source, bus: &mut SnesBus, field: &str, routine: &str) {
    let address = WRAM | (OBJECT + source.symbol(field));
    let value = source.symbol(routine);
    for offset in 0..3 {
        bus.write8(address + offset, (value >> (offset * 8)) as u8);
    }
}

#[test]
fn delay_initializers_and_ticks_match_original_every_count_and_relative_branch() {
    let source = Source::load();
    for (name, native) in [
        (
            "DELAYEXPLODE_ISTRAT",
            delayexplode_init as fn(&mut Game, u16),
        ),
        ("DELAYEXPLODE_STRAT", delayexplode_strat),
        ("BOSSDELAYEXPLODE_ISTRAT", strat_boss_delay_explode_init),
        ("BOSSDELAYEXPLODE_STRAT", bossdelayexplode_strat),
        ("DELAYREMOVE_ISTRAT", delayremove_istrat),
        ("DELAYREMOVEREL_ISTRAT", delayremoverel_istrat),
        ("DELAYREMOVE_STRAT", delayremove_strat),
    ] {
        for free in [false, true] {
            for relative in [false, true] {
                for count in 0..=u8::MAX {
                    let mut game = Game::new();
                    let id = game.objs.alloc().unwrap();
                    let dummy = game.objs.alloc().unwrap();
                    game.vars.dummyobj = dummy as i16;
                    while game.objs.active_indices().len()
                        < game.objs.aliens.len() - usize::from(free)
                    {
                        game.objs.alloc().unwrap();
                    }
                    let next_free = game.objs.free_head;
                    let explode = game.world.register_strategy(strat_explode);
                    let temp = game.world.register_strategy(theend_flyaway_strat);
                    game.vars.pviewvelz = if relative { -65 } else { 65 };
                    let object = &mut game.objs.aliens[id as usize];
                    object.hp = 17;
                    object.ap = 23;
                    object.count = count;
                    object.sflags = 8;
                    object.sflags2 = 0xA0;
                    object.sflags4 = if relative {
                        sf_game::alien::ASF4_RELEXPLODE
                    } else {
                        0
                    };
                    [object.rotx, object.roty, object.rotz] = [251, 252, 253];
                    [object.worldx, object.worldy, object.worldz] = [32760, -32760, -32760];
                    object.expstratptr = Some(explode);
                    object.tempstratptr = relative.then_some(temp);
                    let object = *object;
                    let mut bus = SnesBus::new(source.rom.clone());
                    source.seed(&mut bus, &object, &game);
                    source.byte(&mut bus, OBJECT, "AL_FLAGS", object.flags);
                    source.byte(
                        &mut bus,
                        DUMMY,
                        "AL_COLLFLAGS",
                        game.objs.aliens[dummy as usize].collflags,
                    );
                    source.word(&mut bus, 0, "DUMMYOBJ", DUMMY as i16);
                    source.word(&mut bus, 0, "ALLST", OBJECT as i16);
                    source.word(
                        &mut bus,
                        0,
                        "ALFREELST",
                        if free { CHILD as i16 } else { 0 },
                    );
                    install(&source, &mut bus, "ALX_EXPSTRATPTR", "EXPLODE_ISTRAT");
                    if relative {
                        install(
                            &source,
                            &mut bus,
                            "ALX_TEMPSTRATPTR",
                            "THEEND_FLYAWAY_STRAT",
                        );
                    }
                    source.run(&mut bus, name);
                    native(&mut game, id);
                    let context = format!("{name} free={free} relative={relative} count={count}");
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
                    // Expired boss-delay entries always request the oversized
                    // envelope, even when only the fallback actor is available.
                    if name.starts_with("BOSS") && (count == 0 || count > 128) {
                        let (slot, base) = if free {
                            (next_free.unwrap(), CHILD)
                        } else {
                            (dummy, DUMMY)
                        };
                        let mut actual = game.objs.aliens[slot as usize];
                        assert_eq!(
                            actual.visual_kind,
                            ObjectVisualKind::ExplosionEnvelope(ExplosionSize::Oversized)
                        );
                        let address = WRAM | (base + source.symbol("AL_SHAPE"));
                        actual.shape =
                            u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
                        source.compare_at(&mut bus, base, &actual, &context);
                        if free {
                            assert_eq!(actual.hp, 0, "scheduled initializer has not run");
                            let init = actual.stratptr.unwrap();
                            // Give the envelope one live delay visit; both
                            // sides retain the same continuation afterward.
                            source.byte(&mut bus, CHILD, "AL_COUNT", 1);
                            game.objs.aliens[slot as usize].count = 1;
                            source.run_at(&mut bus, "DELAYEXPLODE_ISTRAT", CHILD);
                            game.call_strat(init, slot);
                            actual = game.objs.aliens[slot as usize];
                            actual.shape =
                                u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
                            source.compare_at(&mut bus, base, &actual, &context);
                        }
                    } else {
                        assert_eq!(game.objs.free_head, next_free, "{context}: no allocation");
                    }
                }
            }
        }
    }
}
